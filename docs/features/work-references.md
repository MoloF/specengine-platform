---
class: spec
status: shipped
shipped: 2026-11-23
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval, plugin, scripts, ui]
ref: RM-0137; the owner's request of 2026-11-21 and his decisions of 2026-11-22 (08 s2 Phase 7, "Owner's word, 2026-11-21/22"); the analyst's breakdown of 2026-11-22, slice 2 of 3, every working answer decided on his "start and go to the end"; plugin 0.51.0
adrs: [ADR-0088]
---

# work-references: photos, sound, models and links attached to work, reaching its agent (RM-0137)

## Why

A writer finds a photo of a location, an artist a model, a sound designer a recording, and none of it could sit beside the task it frames: it lived in chats and drives, and the agent that executes the task never saw it. The owner asked to attach such material to a task and use it later (2026-11-21). On 2026-11-22 he decided that uploads may live on the hub with no off-host copy, accepting that they are lost with the VPS.

A reference is therefore queue data beside its work, as a comment is (ADR-0087): never truth, never in git. Its file lives in a content-addressed store in the data directory. Images reach the agent as images; other media reach it through a required "what matters here" text. Slice 2 of three: `comments` (RM-0136, 0.50.0), `work-references` (0.51.0), `reference-promotion` (RM-0138, a reference into git through the one writing door).

Canon: `docs/canon/work-references.md` (the rule, the store, types, facts, serving, agents, delete and collection, the hub, backup, limits) and `docs/canon/work-references-surfaces.md` (documents, CLI, MCP, HTTP, events, the context lines, the screen, deploy). Decision: ADR-0088. The decisions table (D1-D21) and the full formats are in git history.

## Data

Every format moved into the canon; each part here names where.

### Queue schema 16

`STEP_16`: `refs`, 20 columns (`REF_COLUMNS`), indexes `refs_on`, `refs_b3`, `refs_agent_b3`; a delete an `UPDATE`. `work-references.md` "Store and backup"; `proposal-queue.md` "Store".

### Dump format 9

`STATE_FORMAT` 9, header 11 keys with `refs`, rows after `comment_versions`, no file bytes: `queue-backup.md` "Format".

### The blob store

Layout, the write, the caps and settings, the slots: `work-references.md` "The blob store".

### Types

The thirteen media types, five classes, refusals and `[references] allow`: `work-references.md` "Types".

### Facts

`work-references.md` "Types", **Facts**.

### The agent copy

`work-references.md` "Agents"; the page's making of it: `work-references-surfaces.md` "Screen".

### Documents

A reference (21 keys), a list, a stored blob (10 keys), the text: `work-references-surfaces.md` "Documents".

### CLI

`work-references-surfaces.md` "CLI".

### MCP

`list_references`, `get_reference` (its image block), `add_reference`: `work-references-surfaces.md` "MCP"; counts `mcp-read.md`, `hub-mcp.md`.

### HTTP

Five routes, the binary door, statuses, serving: `work-references-surfaces.md` "HTTP", `work-references.md` "Serving"; the fence's `no-store` exception: `hub.md` "Routes".

### Events and feed

`reference.created`, `reference.deleted`: `work-references-surfaces.md` "HTTP"; `spec-feed.md`.

### A card's context and the task package

`work-references-surfaces.md` "A card's context and the brief"; `card-context.md`, `task-package.md`.

### UI

The References part, uploads, the agent copy, Delete: `work-references-surfaces.md` "Screen".

### Soak RF

The row `write-memory RF` in `soak-runs.md` "Budgets", read by WM6.

## Acceptance criteria

Each criterion names the mutation that must turn it red (M).

- [x] RF1 — Schema step: a schema-15 queue holding proposals, cards, comments and events steps to 16 in one transaction; `PRAGMA user_version` reads 16; `refs` and its three indexes exist; `REF_COLUMNS` (20) equals `PRAGMA table_info`; a failure inside the step leaves the queue at 15. M: a column dropped from `STEP_16`.
- [x] RF2 — IDs and targets: two `spec ref add --on C-0001 --url https://example.org/a` store `RF-0001`, `RF-0002`; an alias exits 1 naming the canonical ID; a look-alike exits 2 with its Latin form; `C-0099` and `PR-0099` exit 1; a node's `on_rev` equals the index's revision; with `[ids]` claiming `RF`, every `spec ref` command exits 2 and `POST .../references` answers 503; no refusal stores a row, a file or an event. M: the card existence check skipped.
- [x] RF3 — Types by sniffed bytes: the type fixtures (PNG, JPEG, GIF, WebP, WAV, FLAC, Ogg Vorbis, Ogg Opus, MP3 with and without ID3, MP4, WebM, GLB 2, `.blend`, PDF) store with their media types and classes. An SVG, an XML file, an HTML page, the same page named `harbour.png`, a ZIP, a gzip, a zstd `.blend`, a tar, an ELF and a `#!` script are each refused (415 on a hub, exit 1 on a terminal) with their line, leaving no file and no `.part`. With `allow = ["image"]` the WAV is refused and the PNG stored; `allow = ["image/svg+xml"]` exits 2 naming its line. M: the type taken from the name's extension (the page named `.png` stored).
- [x] RF4 — Caps before and while reading: with `SPECENGINE_BLOB_FILE_MAX=1048576`, a PUT declaring 1 048 577 bytes gets 413 and no `.part` is created; with `SPECENGINE_BLOB_QUOTA=3145728` and 2 MiB stored, a 2 MiB PUT gets 413; with the free-space seam reporting the floor + 64 MiB at the start and the floor − 1 after 16 MiB, a 64 MiB PUT gets 413 at 16 MiB and its `.part` is gone; the 20 001st distinct file gets 413; `SPECENGINE_BLOB_QUOTA=20000000000` and `SPECENGINE_BLOB_FLOOR=1` exit 2 at the daemon's start. No refusal stores anything or emits an event. M: the floor read only at the start.
- [x] RF5 — Memory while uploading: one 200 MiB PUT to a release hub, its body generated as sent, raises the daemon's RSS (sampled every 50 ms) at most 16 MiB over its value before; the stored file's BLAKE3 equals the generator's. M: the body collected whole before hashing (`to_bytes`).
- [x] RF6 — Content addressing and dedup: a stored file's name equals the BLAKE3 of its bytes, mode 0400; two PUTs of equal bytes answer `stored: new` then `existing`, the store holding one file whose mtime is the second's; two references to it share it; deleting one keeps the file, deleting the other removes it in that call. M: a dedup writing a second file.
- [x] RF7 — Serving: `GET .../blobs/<b3>` of a PNG answers `Content-Type: image/png`, `X-Content-Type-Options: nosniff`, `Content-Security-Policy: sandbox`, `Content-Disposition: inline; filename=…`, `ETag: "<b3>"`, `Cache-Control: private, no-cache`; of a GLB and a PDF `attachment` with `filename` and `filename*` from the stored name; `If-None-Match` equal gets 304; a hash no live reference of the project names (unattached, deleted, another project's) gets 404; it answers while a test holds the project's turn. M: `inline` for a PDF.
- [x] RF8 — Range: on a 3 MiB file, `bytes=1048576-2097151` answers 206 with `Content-Range: bytes 1048576-2097151/3145728` and exactly those bytes; `bytes=-100` the last 100; `bytes=3145728-` 416 with `Content-Range: bytes */3145728`; `bytes=0-0,5-9` and a malformed header the whole file, 200. Ten concurrent readers of a 200 MiB file raise RSS at most 16 MiB. M: the file read whole into the answer.
- [x] RF9 — "What matters": a WAV, an MP4, a GLB or a PDF attached without `matters`, or with a blank one, gets 400 with `matters: required for audio, video, models and PDF: say what matters here`, nothing stored; with it, stored; an image and a link store without it; `spec ref add sound.wav --on C-0001` exits 1. A `.blend` stores with its `warning` and serves as an attachment. M: the rule dropped for `model`.
- [x] RF10 — Facts: the fixtures give PNG 4032x3024; a JPEG whose EXIF thumbnail precedes its frame 1600x1200; GIF 320x240; WebP lossy, lossless and extended their sizes; WAV `duration_ms` 2000; FLAC 1500; Ogg Vorbis 3000; Opus 2500 (pre-skip subtracted); a GLB of an indexed mesh of 12 triangles and an unindexed one of 4: 16; a GLB whose JSON chunk is 1 MiB + 1: `triangles` `null`; MP3, MP4, WebM: `duration_ms` `null`; a truncated PNG: `width` `null`. A counting reader shows no file read past 2 MiB. M: a JPEG's size taken from its EXIF thumbnail (a byte scan for `FF C0`).
- [x] RF11 — Images for agents: `get_reference` on a 1200x800 image of 300 KiB answers its text, then one image block whose decoded bytes are the file's and whose `mimeType` is its media; on a 4032x3024 image with a 1568x1176 copy of 400 KiB, the copy's bytes; without a copy, the text alone with `image: none for agents: …`. `agent_b3` naming a 1600 px copy, a 1.1 MiB copy, another aspect (not the transpose, nor a full-size copy within one pixel an edge), or given for an agent-ready image, gets 400 each. The text block is byte-equal to `spec ref get RF 2>&1`. `get_card {context: true}` on a card with 5 agent-ready images attaches the newest 3, in the `context.images:` order. M: the original's bytes sent over the limits.
- [x] RF12 — Delete rights, on a hub whose `[team]` names an owner, a lead and a member: the author's login and the `owner` get 200; the lead, the member and an outsider 403 with the line; with no or a bad `[team]`, only the author; `spec ref delete` with stdin not a terminal exits 2 and a declined prompt exits 1, both with no event; `tools/list` holds no delete tool, stdio or hub; `GET .../team` lists `delete_reference` for the owner alone. M: any `[team]` login may delete.
- [x] RF13 — Erasure and collection: a reference whose title, "what matters" and name hold unique markers is deleted, then `wal_checkpoint(TRUNCATE)`: no marker in the database file, the WAL or `spec export state`; the row keeps `deleted_by`, `deleted_at`; its file and copy, named by no other live row, are gone in the same call. With `.swept` over 1 h old, an upload sweeps an unattached file 24 h + 1 s old and a `.part` 6 h + 1 s old, keeping a 23 h one. A `POST` naming a candidate between the sweep's outside scan and its turn keeps the file. M: the re-check inside the turn dropped.
- [x] RF14 — Nothing blocks: with 50 references on a card, its move, accept, cancel and clear give the same documents and events as with none; `change_card`, `card_accept`, `approve` and a plain `card show` execute no statement on `refs` (a statement trace). M: `change_card` reads `refs`.
- [x] RF15 — Context and package: on a card with 12 references (5 images, 2 audio, 5 links), `spec card show --context` and `get_card {context: true}` list the newest 10 within 4 096 characters, oldest first, with the note for the rest; a 2 048-byte `matters` shows cut at 512 with ` [cut]`; `U` grows by the lines' characters + 1; `bundle_hash` equals the hash with no references; `get_task` on a task two cards list carries both cards' references, each naming its card, `schema_version` 1. M: the lines left out of `U`.
- [x] RF16 — Events and feed: each attach and delete adds exactly one `reference.*` event whose payload holds no title, URL, name, hash or "what matters"; a PUT adds none; `git status`, `HEAD` and the files are unchanged; `spec feed` words both types. M: the title in the payload.
- [x] RF17 — Backup: a queue with link, file, deleted and node references dumps as format 9 (11 header keys, `refs` counted), imports into an empty data directory and dumps again byte-identical; the dump's size does not change with the files' sizes; a format 8 fixture imports with no references; after an import with no blob store, a file reference reads `file: lost`, its GET is 404 and `spec ref get --out` exits 1 with its line; `docs/generated/queue.md` is byte-identical with and without references. M: `refs` left out of the dump.
- [x] RF18 — The fence and the binary door: the three writes are in `hub.rs` `class` and `GET_ROUTES` pins 41. A PUT with `Content-Type` `multipart/form-data`, `application/json` or `image/png`, or `Content-Encoding: gzip`, gets 415; no `Origin` or a foreign one 403; `Sec-Fetch-Site: cross-site` 403; no `Content-Length` 411; each before a byte is read, no `.part`. `OPTIONS` answers no `Access-Control-*`. A login's third concurrent upload and the hub's ninth get 429 with `Retry-After: 5`; a dropped upload frees its slot, so the next succeeds. M: the slot kept after a dropped body.
- [x] RF19 — Turn fairness: while two 200 MiB PUTs stream to a hub, `GET .../cards` and `add_comment` each answer within 1 000 ms, and a turn trace shows each PUT holding the turn only for its rename and sweep. M: the body streamed inside the turn.
- [x] RF20 — MCP surface: stdio lists 38 tools, a hub 24 (`HUB_TOOLS`), `HUB_READ_TOOLS` 10 with `list_references` and `get_reference`; `list_references`' content is byte-equal to `spec ref list … 2>&1` on stdio and a hub; `add_reference` stores a link authored `{type: agent, role, model, run}`, plus `login` on a hub; an argument `b3`, `file` or `as` is refused by the schema; every hub text is ASCII and at most 2 048 B and calls references people's material. M: `as` added to `add_reference`.
- [x] RF21 — UI: the References part renders under a card (modal and page), a document and a proposal; an image tile's `src` is `.../blobs/<agent_b3 or b3>`; audio and video use `<audio controls preload="none">` and `<video controls preload="metadata">`; a model and a PDF are download links, a `.blend` with its warning; a link carries `rel="noopener noreferrer nofollow"`; no `dangerouslySetInnerHTML` (`policy.test.ts`); progress follows `upload.onprogress` and Cancel aborts; a picked 4000 px image puts its copy before the original; a 400 keeps the dialog and the uploaded file; Delete shows `ActOff` without `delete_reference`; first load under 845 000 B. M: the original posted as its own copy.
- [x] RF22 — Compose and image (`deploy.rs`): the compose file declares `specengine-blobs` and mounts it at `/data/specengine/blobs` on `specengine` alone; `specengine` passes the three settings through bare; `.env.example` lists them commented; the Dockerfile's runtime stage creates `/data/specengine/blobs` owned 10001:10001, mode 0700, before `USER 10001:10001`; `docker compose config` passes. M: the volume mounted on `dump-queue`.
- [x] RF23 — Soak "RF" ("Soak RF"), two runs of 300 s on release binaries at realistic rates, 25 MiB/s per uploader (RF19's) and 10 Range windows a second per reader, both green against the row's bars; a rate override is a diagnosis, never evidence; the row (`soak-runs.md` "Budgets", `write-memory RF`) and `Variant::RF` landed together, WM6 green. Passed 2026-11-23: "Release". M: a planted leak keeping 64 KiB per PUT: red, live heap 14.809 MiB/min, positive in every third (the quiet footprint alone read 2.114).
- [x] RF24 — Plugin version: `plugin.json` 0.51.0 with `PINS` and `SCHEMA_PINS` appended; the skills name the three tools within `plugin-files.md`'s budgets. M: the schemas change while the version stays 0.50.0 (`tool_schemas_follow_the_plugin_version`).
- [ ] RF25 — Owner's gate, an upload through Traefik: on the live hub, upgraded after a dump ("Hub upgrade"), `stat -c '%u:%g %a' /data/specengine/blobs` in `specengine` reads `10001:10001 700`; a 100 MiB WAV or MP4 uploads through Traefik within its 300 s `readTimeout` (about 2.7 Mbit/s up at least); its reference shows, and the downloaded file's `b3sum` equals the original's. It blocks nothing (ADR-0012).
- [ ] RF26 — Owner's gate, an image to Claude Code: an image reference on a card reaches Claude Code over the hub's `/mcp`: `get_card {context: true}` attaches it and the agent describes what it shows; `get_reference` does too. It blocks nothing.

## Out of scope

Editing a reference (delete and add again); uploads by agents or over MCP; any server-side decoding, thumbnail or transcode; previews of models and PDFs; virus scanning; per-file or per-login access; fetching or checking a link; quotas per login; several ranges in one answer; references in `get_node`, the bundle, search, the `cited` scan, `spec history` or `spec delivery`; a count on a board tile; a `spec check` code for `[references]`; off-host copies of files; merging a terminal's store with a hub's; promotion into git (RM-0138); any change to a card's `evidence`.

## Pins that move

Moved by this slice (all landed): the index shard `docs/index-canon-work.md` (`comments*`, `work-*`) in `shards_repo.rs` `SHIPPING`, `POINTERS` and `parity_config` (`INDEX_LIVE_SHARDS` 12, `INDEX_OUTPUTS` 15), `check_parity.rs`; `QUEUE_SCHEMA_VERSION` 16, `STATE_FORMAT` 9, `REF_COLUMNS` 20, `QUEUE_EVENT_TYPES` 30, `GET_ROUTES` 41, `HUB_TOOLS` 24, `HUB_READ_TOOLS` 10, stdio 38 (`KNOWN_TOOLS`), five key sets in `fixtures/daemon-keys.json`, an owner's `allowed` ending `delete_reference`, `PINS` and `SCHEMA_PINS` at 0.51.0; `fixtures/blob-types/`.

## Implementation

| Module | What it does |
|---|---|
| core `blob.rs`, `reference.rs`, `project_toml.rs`, `team_toml.rs` | caps and settings (`BlobCaps`), `sniff`, `facts`, `agent_ready`, `agent_copy_problem`, `Intake`, `BlobHasher`, `size_text`; `RF-` IDs and the `[ids]` clash, texts' caps, `ReferencesConfig`; `[references]` name-checked; `delete_reference` in an owner's `allowed` |
| model `task.rs` | `PackageReferences`: the package's `references` |
| store `queue.rs`, `queue/refs.rs`, `queue/state.rs`, `queue/comments.rs` | `STEP_16`, schema 16; add, delete (erasing), get, list, the card context, each read stepped; `BlobReader`; `each_live_blob` for the sweep; the two events; the dump's `refs` |
| cli `blob_store.rs`, `reference.rs`, `card.rs`, `package.rs`, `feed.rs`, `state.rs`, `state_file.rs`, `main.rs`, `lib.rs` | the store's write, flight, walk, sweep, `statvfs` (the crate's one unsafe module); `spec ref add/list/get/delete`, `blob_begin`, `blob_commit`, `blob_read`, the delete's judge and collection, the context lines and images; the brief's "References" (`SECTIONS` 14); the feed's words; `STATE_FORMAT` 9 |
| mcp `references.rs`, `server.rs`, `hub.rs`, `cards.rs`, `mirror.rs` | the three tools, the image block, `get_card`'s images, `HUB_TOOLS` 24, `HUB_READ_TOOLS` 10, `HUB_INSTRUCTIONS` 2 045 B, the mirrors |
| http `blobs.rs`, `app.rs`, `answer.rs`, `hub.rs`, `main.rs` | the binary door, slots, the stream to one blocking writer, early answers, serving and Range; the reference routes; the fence's `ServedFile` exception; `class`; caps read at start |
| eval `write_soak.rs` | the `RF` variant, in WM6 |
| ui `src/references/`, `api/referencesHttp.ts`, `referenceQueries.ts`, `client.ts`, `http.ts`, `mocks/references.ts`, `i18n/enReferences.ts`, `ru.ts`, board, doc, inbox, `vite.config.ts` | the References part, `putBlob`, the agent copy, the live re-reads, the mock, the lazy English part, the dev proxy's `Origin` on `PUT .../blobs` |
| plugin, `scripts/deploy/{docker-compose.yml,Dockerfile,.env.example}` | 0.51.0, `implement` names the three tools (3 912 B); the volume, its mount point, the three settings |

**Deviations from this spec:**
1. A PUT carries no file name: its refusal lines say `the upload`, the `.blend` hint is a terminal's alone; `POST`'s `allow` refusal names the file.
2. `author` is the core `Author`, as comments'; an agent copy is served with its own sniffed type.
3. Error bodies: a `POST`'s 400 `{status, message}` with `spec: …`; 411, 413, 415, 429 `refused: …`; a GET's 404 `{id, reason}`. A PUT's 400 also covers a malformed length, a query, an idle (60 s), broken or short body; a writer's panic is 500.
4. `If-None-Match` also matches `W/"<b3>"` and `*`.
5. The quota counts the sizes in-flight uploads declared, the flight taken before the walk (iteration 3); the file count is judged at the rename, in the turn; `SWEEP_MAX` counts stale files and `.part` files together.
6. The blob GET reads through CLI `blob_read` (`BlobReader`), each 64 KiB piece a short blocking read, no thread held while a reader idles.
7. The fence writes `no-store` on every response but a served file's (the blob GET's 200, 206, 304).
8. Early refusals read at most 1 MiB for 2 s, then close; with `Expect: 100-continue` and the body unread, at once (iteration 3). A larger refused body sent straight to the daemon may still see a reset; the page retries a status-0 PUT once.
9. The agent copy also passes a full-size copy within one pixel an edge (`|cw·oh − ch·ow| ≤ max(ow, oh)`, as is or transposed; iteration 3), for strips a browser rounds.
10. Sizes print to the nearest tenth; the floor's remainder rounds down. `--name` or `--agent-copy` with `--url` exit 2 (clap). Facts read 8 KiB windows within 2 MiB.
11. Stdio `INSTRUCTIONS` stay 2 043 B and do not name the three tools (no room); `HUB_INSTRUCTIONS` gained its line by cuts (`, archive?`, `, limit?`, five phrases).
12. `context.images:` joins its IDs with `, `.
13. UI: the copy is judged on header bytes the page reads (none when within the limits or of unknown size, none above 64 MP); an `agent_b3:` 400 re-posts once without it; thumbnails only from a copy or an agent-ready original, else "Open image (W×H, size)"; the viewer draws an original within 64 MP (32 MiB when unknown); a 429 waits at most 60 s, 5 tries.

## Release

Plugin 0.51.0 (MINOR), tag `v0.51.0`; queue schema 16, `STATE_FORMAT` 9, `INDEX_FORMAT` unchanged at 21; the hub's upgrade takes a dump first, adds `specengine-blobs` and every binary comes from the tag (`hub-ops.md` "Upgrade", `scripts/deploy/README.md`). First load 816 857 B (815 001 before; bar 845 kB).
- Iterations: Rust three, UI three; reviews ACCEPT.
- The full run: pending.
- RF23, two paced 300 s runs on release binaries (2 uploaders of 200 MiB at 25 MiB/s, 10 Range readers at 10 windows a second, CM's writers; no override; load under 6, `caffeinate -i -s`), both green:
  - run 3: 16 411 calls/min (12 738 writes, 3 673 reads); 58 uploads, 11.3 GiB; 17 862 Range windows, every byte checked; live heap 0.000, quiet footprint −0.547 MiB/min; RSS 5.4 → 31.4 MiB; p50/p95/p99/worst 23/49/53/8 132 ms; page p95 27 → 56 ms (bar 90.5); 0 hangs;
  - run 4: 16 805 (12 987, 3 818); 58, 11.3 GiB; 18 570; 0.000, −0.360; 5.4 → 31.1 MiB; 24/50/53/8 120 ms; 28 → 56 ms (bar 92); 0 hangs;
  - the worst call is one 200 MiB PUT at 25 MiB/s, about 8 s by design; the twins 2.4 % apart; each store ended with its 20 files and no `.part`; the planted leak (64 KiB kept per PUT) red at 14.809 MiB/min on the live heap.
- Before pacing: Unpaced, client and daemon on one laptop (the client about 760 MiB/s of uploads and 1.1 GiB/s of Range windows, load 11-16): memory green (live heap 0.018 and 0.031 MiB/min, quiet footprint 0.031 and −0.656), but the page p95 26 → 237 and 28 → 221 ms against bars 89 and 92, from CPU contention: the page's turn wait p50 18 → 106, p95 23 → 145 ms, its handler ~6 ms, `add_comment` 2 → 12 ms; the uploads' own turn work small (commit p50 10 ms, POST 6, DELETE 10; ~0.1 s of turn a second). A paced diagnostic run: page p95 28 → 60 ms (bar 92), live heap 0.001. The planted leak: red, 14.809 MiB/min.
- Open for the owner: RF25 and RF26 on the live hub; the laptop's reinstall from the tag.
