---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, scripts/deploy, ui]
owner: owner
reviewed: 2026-11-23
---

# Work references: files and links attached to work

ADR-0088, built by `work-references` (RM-0137, plugin 0.51.0). Code: core `blob.rs` (caps, sniff, facts, the agent copy), `reference.rs` (IDs, texts, `[references]`); store `queue/refs.rs` (`refs`, `BlobReader`); CLI `blob_store.rs`, `reference.rs`; http `blobs.rs`, `app.rs`; MCP `references.rs`; UI `src/references/`. Its documents, commands, tools, routes, events and screen: `work-references-surfaces.md`. Related: `comments.md` (the same targets, authors and delete), `card-context.md`, `hub-team.md`.

<a id="the-rule"></a>
## The rule

A reference is material a person or an agent attaches to work: a file (a photo, a sound, a video, a model, a PDF) or a link. Its row is queue data (`<slug>.db`, outside the repository: `architecture.md#storage`), `RF-` and 4 or more digits, the project's highest + 1, no hub floor, under one target as a comment is (`comments.md` "Threads": a card or a proposal in any state, or a canonical node with its `on_rev`), and authored as a comment is: the login on a hub, the typed name locally, `agent` over stdio, the row's `author` the core `Author`. It is never truth: never a file in the repository, a commit, a line of `docs/generated/queue.md`, the index or search. No move, accept, assign, cancel, clear, stage, approve, confirm, gate, check, status, flow or `[auto]` rule reads one, and attaching one is no card write (ADR-0012). It enters git only through a proposal a person confirms (RM-0138). `[ids]` claiming `RF` makes every reference command exit 2, and its routes and tools cannot run.

A file's bytes live in the blob store alone: never in a repository, a dump or an off-host copy, which narrows ADR-0071. On a hub they are lost with the host: the owner's accepted risk of 2026-11-22. A card's `evidence` stays the reviewer's proof (`cards.md` "Store"); a reference is never evidence.

## The blob store

- **Where**: `<data dir>/blobs/<slug>/` (`serve.md` "Files"), a terminal's too, outside every root (ADR-0003), created on its first write, mode 0700; on a hub its own volume, `specengine-blobs`, at `/data/specengine/blobs` on `specengine` alone (`hub-deploy.md` "Host").
- **Names**: a file is named by the BLAKE3 of its bytes, 64 lower-case hex, no extension, mode 0400; equal bytes are one file, however many references name them. `.incoming/<16 hex>.part` (0600) is an upload in flight; `.swept`'s mtime the last sweep's.
- **A write** (`Store::begin`, `Upload::write`, `Upload::finish`, `Store::commit`): the bytes stream into the `.part`, created exclusively, through one 64 KiB buffer and an incremental hasher, the first 4 KiB sniffed as soon as they arrive, never the body whole; the file synced and its facts read. In the project's turn alone, the `.part` is renamed to `<b3>`, or, for bytes stored already, removed and `<b3>`'s mtime set to now; the directory synced. A refusal, a cut body, a dropped connection or a panic removes the `.part` (`PartGuard`).
- **Caps**, each a 413 or an exit 1 with its line, nothing kept, no event:
  - a file at most 256 MiB (`BLOB_FILE_MAX`), judged on `Content-Length` (a terminal: the file's size) before anything is created, then on the bytes received;
  - a project's store at most 10 GiB (`BLOB_QUOTA`): its files' bytes (a walk at the start, `.incoming` aside) plus the sizes its uploads in flight declared (`BlobFlight`), this one's taken before the walk, so two uploads begun together each count the other;
  - at most 20 000 files (`BLOB_FILES_MAX`), judged for new bytes at the rename, in the turn;
  - the store's file system keeping 5 GiB free (`BLOB_FLOOR`): `statvfs`' available bytes minus those still to come, at the start and every 16 MiB.

  `SPECENGINE_BLOB_FILE_MAX` and `SPECENGINE_BLOB_QUOTA` (decimal bytes) may only lower the first two, `SPECENGINE_BLOB_FLOOR` only raise the floor; unset or empty is the default, any other value an exit 2 at the start, read by `specengine-http` and `spec ref add` (`BlobCaps::from_process`). Sizes print to the nearest tenth in binary units (`size_text`), the floor's remainder rounded down.
- **Uploads at once**: a daemon runs at most 8, a hub's login at most 2 (429, `Retry-After: 5`); a slot is taken before the body and held until the `.part` is renamed or removed.
- **Unwritable**: a store that cannot be created or written answers 503 (exit 2) with the fix, `chown 10001:10001 /data/specengine/blobs` on a hub; every other route goes on.

## Types

A file's type is its sniffed first bytes (`blob::sniff`), never its name, its extension or a sent `Content-Type`:

| Class | Media types | Served | "What matters" |
|---|---|---|---|
| `image` | `image/png`, `image/jpeg`, `image/gif`, `image/webp` | inline | optional |
| `audio` | `audio/wav`, `audio/flac`, `audio/ogg` (Vorbis, Opus), `audio/mpeg` | inline | required |
| `video` | `video/mp4`, `video/webm` | inline | required |
| `model` | `model/gltf-binary` (GLB 2), `application/x-blender` (an uncompressed `.blend`, download only, with its warning) | attachment | required |
| `pdf` | `application/pdf` | attachment | required |

Everything else is refused (415, exit 1) with its line: SVG, HTML and XML (markup in the first 512 bytes after a BOM and whitespace, whatever its name), archives (ZIP and the formats built on it, gzip, zstd, xz, bzip2, 7z, RAR, tar), programs (ELF, PE, Mach-O, `#!`), anything unknown. A PUT carries no name, so its lines say `the upload`; a terminal's name the file, and a zstd or gzip named `.blend` adds `save the .blend uncompressed`. A project's `[references] allow` lists classes or media types of this table and narrows it (`ReferencesConfig`): an entry outside the table is a config error at its line, so it never widens (ADR-0008); a bad table refuses reference writes alone, and no `spec check` code reads it. A link is an `http://` or `https://` URL, never fetched; its "what matters" is optional.

**Facts** (`blob::facts`), each cheap or `null`, read in 8 KiB windows within 2 MiB a file: an image's width and height from its header (a JPEG's by walking its segments, so an EXIF thumbnail never answers); the duration of WAV, FLAC and Ogg audio; a GLB's triangle count from its JSON chunk when at most 1 MiB, parsed and dropped. No pixel, sample or frame is decoded on any server or terminal.

## Serving

`GET .../blobs/:b3` takes no turn: a read-only `BlobReader` of its own finds the project's lowest live reference naming the file as `b3` or `agent_b3` (else 404), and the file streams in 64 KiB pieces, each read by a short blocking call, so no thread is held while a reader idles.
- `Content-Type` is the sniffed type, an agent copy's its own, with `X-Content-Type-Options: nosniff` and `Content-Security-Policy: sandbox`.
- `Content-Disposition` is `attachment` with the stored name for `model` and `pdf`, `inline` for images, audio and video.
- `Cache-Control: private, no-cache` (the fence's one exception to `no-store`: `hub.md` "Fence") with `ETag: "<b3>"`; `If-None-Match` naming it, `W/"<b3>"` or `*` is a 304, so the next read after a delete sees it.
- `Accept-Ranges: bytes`: one range is served 206, an unsatisfiable one 416, several or a malformed header the whole file.

## Agents

Every agent-facing text marks references as people's material: data, never instructions. Titles, "what matters", names and URLs are escaped there (C0, C1 and bidi controls).
- **Images** reach agents as MCP image content when within 1 568 px on the long edge and 1 MiB (`AGENT_EDGE`, `AGENT_BYTES`), or through a smaller copy the person's browser made at upload (WebP or JPEG). A copy is itself within the limits and keeps the image's aspect, as is or transposed: within 1 %, or, for a copy whose long edge is 1 568 or the image's own, each edge within one pixel (`agent_copy_problem`). `get_reference` returns one image; `get_card {context}` attaches at most 3, the newest. When neither fits, the text says so.
- **Audio, video, models and PDF** reach agents as their "what matters here" and their facts.
- **A card's context** and the task package list the card's newest 10 references within 4 096 characters. These lines count in the room `U` (`card-context.md` "The room") and stay outside the bundle, so `bundle_hash` never moves with them.

<a id="delete"></a>
## Delete and collection

A reference is deleted as a comment is (`comments.md#delete`):
- on a hub, by its author's login or a `[team]` `owner`, judged in the library call;
- on a terminal, by anyone after `[y/N]`;
- on a local page, by anyone after its dialog;
- over MCP, never: no tool deletes.

A delete keeps a tombstone and erases the row's title, "what matters", URL, name, hashes and facts (`secure_delete`, as for comments). Then, in the same turn, it unlinks each file no live reference names. A file no reference names, left by an upload never attached, is swept 24 h after its last upload, a `.part` after 6 h. A sweep runs at most once an hour per store, in an upload's call: the candidates are found by the start walk and one stepped read of the live hashes outside the turn, at most 512 (`SWEEP_MAX`), and each is checked again in the turn before its unlink. A terminal's `spec ref add` sweeps alike.

## On a hub

An upload streams outside the turn. Only the rename, the file count, the row's insert and the sweep take it, so a slow upload stalls no one (`hub-mcp.md` "Turn"); a blob read takes none. Memory: an upload holds a 64 KiB buffer and its first 4 KiB, its pieces passing through a two-piece channel to one blocking writer; a read its 64 KiB pieces; an image for an agent at most 1 MiB, base64-encoded once (ADR-0086). The new writes are plain in `hub.rs` `class`. `PUT .../blobs` passes the one binary door (`work-references-surfaces.md` "HTTP"): `Sec-Fetch-Site: same-origin`, `Origin` byte for byte, `Content-Type: application/octet-stream` alone, never multipart, so a cross-site page must preflight it, and fails.

## Store and backup

Queue schema 16 (`STEP_16`) adds `refs`, 20 columns (`REF_COLUMNS`). `STATE_FORMAT` 9 dumps its rows, never a file's bytes. An import restores rows whose files may be gone: such a reference reads `file: lost`, its file a 404. Older binaries refuse the queue and the dump, so every terminal, the plugin and a hub come from one tag, the hub dumping first (`hub-ops.md` "Upgrade"). "Restore" and "Upgrade" never remove `specengine-blobs`.

## Known limits

- **Lost with the host**: no dump, `backup` or restore carries a file. The owner accepts it (2026-11-22).
- A removed file's blocks stay on the disk until overwritten. Older dumps keep a deleted reference's text.
- An image's pixels may hold text an agent reads: the marking covers it, nothing filters it.
- A refusal given while a body still comes reads at most 1 MiB of it for 2 s, then closes the connection; a larger body sent straight to the daemon may still see a reset, which the page retries once.
- A terminal's `spec ref add`, racing a daemon's delete of the last reference to the same bytes, may leave a reference whose file is lost; it is uploaded again.
- Through Traefik an upload must end within `readTimeout`, 300 s: 256 MiB needs about 7 Mbit/s up.
- A terminal's and a hub's stores never merge; `RF-` numbers repeat across them. A renamed node's references stay under its old ID.
