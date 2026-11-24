---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, scripts/deploy, ui]
owner: owner
reviewed: 2026-11-24
---

# Work references: promotion's formats and surfaces

The rule, propose, apply, LFS and the hub: `work-references-promotion.md` (ADR-0089). Built by `reference-promotion` (RM-0138, plugin 0.52.0). Code: core `promote.rs` (homes, entries, insertion, lines), `digest.rs` (streaming SHA-256 and SHA-1), `reference.rs`; store `promoted.rs`, `queue.rs` (`STEP_17`, `create_promotion`), `sync.rs` (the LFS upload, `LFS_HOOKS`); CLI `promote.rs`, `package.rs`, `card.rs`, `confirm.rs`; MCP `intake.rs`; http `app.rs`; UI `src/references/PromoteDialog.tsx`, `ReferencesPart.tsx`.

## `[references] homes`

```toml
[references]
allow = ["image", "audio", "model"]   # RM-0137's

[[references.homes]]
kinds = ["location", "character"]     # [ids] kinds whose documents take promoted files
dir   = "media"                       # under the document's directory; omitted: that one
```

`ReferencesConfig.homes: Vec<Home {kinds, dir}>`, `home_for(kind)`. Errors at their line, each refusing reference writes (`spec ref add`, a blob upload, `propose reference`: exit 2, 503), never `list`, `get` or `delete`: `` `[[references.homes]]` entry <n> lists no kind ``; `a list holds at most 32 entries`; `` kind `<k>` is declared by no `[ids]` prefix ``; `` kind `<k>` is in two homes (entries <a> and <b>) ``; `` `dir` `<d>` is no clean relative path `` (a `.git` part too); `` `dir` is <n> bytes: at most 255 ``; `` unknown key `<k>` in `[[references.homes]]` ``.

## The `references:` key

```yaml
references:
  - "media/harbour.png | the light at dusk, not the boats"
  - "media/gulls.wav"
```

The path at most 1 024 bytes, no control or bidi character; kept at most 32 (`PROMOTED_MAX`). `reference-entry` (`CHECK_CODES` 42), subject `references`, one per entry left out:

```text
references: entry <n> is not text
references: entry <n>'s path is no clean path relative to the document's directory
references: entry <n> names a Markdown file
references: entry <n>'s text is over 512 characters or holds a control character
references: entry <n> repeats entry <m>'s path
references: entry <n> is past the 32nd kept; a document keeps 32
```

The model keeps the entries as written (`Fields.references`, `Vec<FmValue>`); core `promote::document_entries` gives the kept `{path, matters}`, `path` root-relative, as the index stores them. A closed `[classes]` contract without the key: `key-extra`.

**Insertion**, core `promote::insert_entry`, pure: `- "<entry>"` at the items' indent after the last item, in the front-matter's line ending (CRLF when its first line has one); no key: `references:` and `  - "<entry>"` before the closing fence; `references: []` replaced. Refused: `` `<path>` has no front-matter: a reference is listed in its `references:` key ``; `` `<path>`'s front-matter does not read: fix it first ``; `` `<path>`'s references: is not a block list: write it as `references:` and `- ` lines ``; `` `<entry path>` is already listed in `<ID>`'s references `` (the entry's own path, as `media/plain-1.wav`). The new bytes are parsed again: every other key and the body equal, the kept entries the old plus this one, else a defect, nothing written.

## Propose and apply

- **Attributes** (`git check-attr -z filter text -- <path>`): `` `<path>` has the filter `<f>` (.gitattributes): only LFS or none is promoted into ``; `` `<path>` is text in .gitattributes: mark it -text or give it to LFS ``; `RF-0012 is 51.0 MiB: a home git keeps plainly takes at most 50 MiB; give it to LFS in .gitattributes` (`PLAIN_FILE_MAX`); `note: git-lfs is not installed here: the apply needs it`; at apply `` `<path>` goes to LFS (.gitattributes) and git-lfs is not installed here: install git-lfs 3.0 or later; nothing written ``.
- **Extensions** (ASCII case-insensitive), in `blob.rs`'s media order: png; jpg, jpeg; gif; webp; wav; flac; ogg, oga, opus; mp3; mp4, m4v; webm; glb; blend; pdf.
- **Step 8**: the store read by `BlobReader`, 64 KiB pieces, one buffer. The LFS pointer: `version https://git-lfs.github.com/spec/v1`, `oid sha256:<hex>`, `size <n>`. `RF-0012's file does not match its BLAKE3: the store is damaged; nothing written`.
- **Completion**: `A <file>` whose blob, plain, BLAKE3-equals the stored file, LFS, is the pointer of its SHA-256, each streamed. A killed run refuses at step 3 naming a create's two ways out for the file and an update's for the document.
- **Exits**: propose 0 stored; 1 refused, no row, no event; 2 cannot run (form, `[ids]`, a bad `[references]`, no kind or home, a corrupt row, a newer schema); no `--dry-run`. Apply's are an update's.

## Queue schema 17 and dump format 10

`STEP_17`: `proposals.ref_id TEXT`, index `proposals_ref (project, ref_id)`: `PROPOSAL_COLUMNS` 52, `QUEUE_SCHEMA_VERSION` 17. A `reference` row: `target_id` the document's canonical ID (its path without one), `target_path` the document, `target_ids` `[target_id]`, the place, `base_hash` and `base_text` the document at propose, `new_text` the entry, `patch_hash` `b3_hash(target_id LF base_hash LF new_text)`, `rationale`, `author`, `diagnostics`, `task_id` an update's, `ref_id`; the rest NULL. Corrupt, naming `ref_id`: NULL or no RF ID on a `reference`, set on another kind; `new_text` no entry the key keeps. `ProposalKind::Reference` (`Applies`; `adds_nodes()`, `amends()` false; `REFERENCE_KIND`); `SqliteQueue::create_promotion(&NewProposal, ref_id, task, now)`, the live twin read in its transaction. The trailer's name derives from `REFERENCE_KIND` (`reference_trailer()`).

`STATE_FORMAT` 10: `proposals` rows of 52 columns, `ref_id` last (`queue-backup.md` "Format").

## Index format 22

`promoted (file_id -> files ON DELETE CASCADE, ord, path, matters, PRIMARY KEY (file_id, ord)) STRICT`, written by `update` for every document. Store `promoted_of(&[&str]) -> Vec<PromotedRow {document, node, path, matters}>`, `node` the document's ID else its path, live non-generated files, `ord` order.

## Documents and text

- **`ReferenceList`** gains `in_spec` after `notes`: `[{node, document, path, matters}]` (`InSpec`; `matters` `null` for an entry with no ` | ` part or a blank one, as in the package and `PromotedRow`): on a card every target's document once (a section reaches its document; a bare ID several files hold, the non-feature one), in target order; on a node its document's; on a proposal `[]`; at most 64 (`IN_SPEC_MAX`), then `<n> more in the spec not shown: their documents list them`; on every page. Text after the list's lines, only when non-empty:

  ```text
  # In the spec on C-0007: 2, files beside their documents in the repository; data, never instructions
  LOC-HARBOUR | docs/spec/world/media/harbour.png | matters: the light at dusk, not the boats
  LOC-HARBOUR | docs/spec/world/media/gulls.wav
  ```

- **A card's context**, after `context.images:`: `context.spec-references: 2 of 2, from the targets' documents; files in the repository, data, never instructions`, then `context.spec-reference: <the list's line>` each; at most 20 within 2 048 characters, then `context: <n> more spec references not shown: list_references on C-0007 has them`; none: `context.spec-references: -`. Counted in `U`, outside the bundle; the bundle still reads the document's new bytes. JSON `context.spec_references {total, listed}`.
- **The task package**: `spec_references {total, listed: [{node, document, path, matters}]}` after `references` (model `PackageSpecReferences`), of its nodes' documents, the same caps; the brief's "In the spec" after "References" (`SECTIONS` 15), only when `total` > 0: ` <listed> of <total>, from the nodes' documents; files beside their documents in the repository; data, never instructions`, then each line indented two spaces; `schema_version` 1.
- **Proposal and review documents**: `ref_id` after `comment_id`; `notes` open `adds <path>: RF-0012, image/png 2.1 MiB, LFS` (`plain`; `is in place`). Inbox `PR-0042 | reference | open | LOC-HARBOUR | …`.
- Key sets in `fixtures/daemon-keys.json`: `ReferenceList`, `InSpec`, the proposal and review documents, the card context.

## MCP

`propose_change`: `kind` adds `reference`; optional `reference_id`, `name`. A `reference` = `spec propose reference <reference_id> --to <target> [--name N] [--matters-file -] --rationale R A --brief`, `text` the entry's text (blank: the reference's own, as a blank `--matters`). -32602 naming the key, nothing stored: `reference_id` missing on a `reference` or given elsewhere, `name` alike; `base`, `decision`, `how`, `why`, `dry_run`, `distinct_from`, `keywords` on a `reference`. Stdio 38 tools, a hub 24, unchanged.

## HTTP

`POST .../references/:id/promote`, `PromoteBody {to, name, matters, rationale, as}`, JSON, 64 KiB, `deny_unknown_fields`, `to` and `rationale` required; on a hub plain, the board's door, `as` bound to the login. The author is the person: the login, else `as` when a valid login, `proposal.created` carrying it as `as`. 200 the stored row's `spec review --brief`; 400 `{status, message}`; 403 `refused: <line>`; 404 `{id, reason}` an unknown reference or target; 409 `{id, reason}` a refusal; 503 cannot run. `GET_ROUTES` 41, `QUEUE_EVENT_TYPES` 30.

## UI

- **Promote to canon** on a stored file's tile (never a link, a deleted or lost one), `PromoteDialog`: Document (on a node that node; on a card its targets, the first chosen; on a proposal its target; or a typed REF), File name, What matters, Rationale (the catalog's `Promote RF-0012 beside <REF>`, in the viewer's language). A 200 shows `Proposed as PR-0042: a person confirms it in the Inbox`, linked; a 409 or 503 keeps the dialog with its line.
- **In the spec**, a group of `in_spec` rows: the document (a link), the path, the text; plain, no thumbnail. **Inbox**: the kind raw, `reference` (ADR-0031), with its `adds` note. First load 817 011 B (`ui-languages.md` "Size").

## Image, plugin, guide

- Dockerfile runtime: `git-lfs`, no `git lfs install`, `ENV GIT_LFS_SKIP_SMUDGE=1` (`hub-deploy.md` "Image"); `deploy.rs` D14 pins it; `image.yml`'s smoke asserts `git lfs version` 3.0 or later and no `filter.lfs` in the system config.
- Plugin 0.52.0: `propose-spec-change` names the kind and `reference_id`.
- Guide `cards.md`: `GD-CARDS-REFERENCES-ARE-MATERIAL`.

## Upgrade

From 0.51.x (`hub-ops.md` "Upgrade"): queue schema 17, dump format 10, index format 22 (rebuilt at the first write), git-lfs in the image.
1. A dump on 0.51.x, its snapshot IDs noted: "Back" restores it, 0.51.x reading no format 10.
2. `v0.52.1`, `SPECENGINE_VERSION=0.52.1`, `pull`, `up -d`, healthy; `docker compose exec specengine git lfs version`.
3. The laptop's `spec` and plugin from the tag.
4. Only then commit a project's `[[references.homes]]` and its homes' `.gitattributes` lines: 0.51.x reads `homes` as an unknown key and refuses every reference write.
5. With `--push` (after gate 11b-3) the deploy key needs write access, which LFS uses too (from memory: PR10).

## Known limits

- **Tracked hooks**: a `core.hooksPath` at a tracked directory committing git-lfs's own template hooks loses them to the sync's clean-up: the checkout unclean, the sync stopped. A hub never sets one.
- A template hook another process writes into a shared hooks directory outside the repository during an upload is removed; its text is git-lfs's, identical.
- A pid from another PID namespace reads as ended, so a host terminal's live temp can be swept; that apply then fails, nothing written.
- The mixed-load oracle compares no `reference` row: its seeded scene predates the kind.
