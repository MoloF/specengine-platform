import { ClientError, type ReferencesOptions, type UploadProgress } from "../api/client";
import type { Author, BlobStored, InSpec, PromoteBody, Reference, ReferenceAddBody, ReferenceList, ReferenceMeta } from "../api/types";
import { fakeHex, proposal, stamp, type MockCorpus, type MockDocument, type MockProposal, type StoredReview } from "./build";
import { resolveRef } from "./corpus";

// The mock's references (docs/features/work-references.md "Data", "HTTP"; ADR-0088): harbor-sim's
// material under a task, held in memory until a reload, answered as `spec serve` would: a list oldest
// first in pages of 100 by ID, the newest page first; an upload's bytes sniffed by their first bytes
// (a refusal naming "the upload", as a PUT carries no name),
// kept in memory up to 8 MiB in all, named by a made-up 64-hex digest of their bytes (not BLAKE3, so
// equal bytes are one file); an attach and a delete refused in the CLI's words. A file a seed names
// with no bytes reads `lost`. A promotion (docs/canon/work-references-promotion-surfaces.md "HTTP") stores a
// `reference` proposal in the project's queue, judged as `spec propose reference` judges it, the
// mock's `[[references.homes]]` taking `mechanic` documents into `media`, which its `.gitattributes`
// gives to LFS; a hub's confirm of one adds its entry to the document's key, read back as `in_spec`.

/** The most references a page lists (`REF_PAGE_MAX`). */
const PAGE = 100;

/** The most bytes the mock keeps of every upload together. */
export const MOCK_BYTES = 8 * 1024 * 1024;

/** The agents' limits (`AGENT_EDGE`, `AGENT_BYTES`). */
const AGENT_EDGE = 1568;
const AGENT_BYTES = 1_048_576;

/** A 16x9 PNG, harbour blue over sand: harbor-sim's seeded image. */
const HARBOUR_PNG = "iVBORw0KGgoAAAANSUhEUgAAABAAAAAJCAIAAAC0SDtlAAAAGUlEQVR42mMQMYkhCTEMBw3XFniQhAahBgCHD6LBdMMT+wAAAABJRU5ErkJggg==";

/** The stored types: media type, class, served inline, and its first bytes' test. */
const TYPES: readonly (readonly [string, string, boolean, (bytes: Uint8Array) => boolean])[] = [
  ["image/png", "image", true, (b) => starts(b, 0, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])],
  ["image/jpeg", "image", true, (b) => starts(b, 0, [0xff, 0xd8, 0xff])],
  ["image/gif", "image", true, (b) => text(b, 0, "GIF87a") || text(b, 0, "GIF89a")],
  ["image/webp", "image", true, (b) => text(b, 0, "RIFF") && text(b, 8, "WEBP")],
  ["audio/wav", "audio", true, (b) => text(b, 0, "RIFF") && text(b, 8, "WAVE")],
  ["audio/flac", "audio", true, (b) => text(b, 0, "fLaC")],
  ["audio/ogg", "audio", true, (b) => text(b, 0, "OggS")],
  ["audio/mpeg", "audio", true, (b) => text(b, 0, "ID3") || (b[0] === 0xff && ((b[1] ?? 0) & 0xe0) === 0xe0)],
  ["video/mp4", "video", true, (b) => text(b, 4, "ftyp")],
  ["video/webm", "video", true, (b) => starts(b, 0, [0x1a, 0x45, 0xdf, 0xa3])],
  ["model/gltf-binary", "model", false, (b) => text(b, 0, "glTF")],
  ["application/x-blender", "model", false, (b) => text(b, 0, "BLENDER")],
  ["application/pdf", "pdf", false, (b) => text(b, 0, "%PDF-")],
];

/** The classes whose "what matters" is required. */
const MATTERS = new Set(["audio", "video", "model", "pdf"]);

const BLEND_WARNING = "a .blend is only downloaded here; Blender can run the scripts it holds: open it with Auto Run Python Scripts off";

function starts(bytes: Uint8Array, at: number, magic: readonly number[]): boolean {
  return magic.every((byte, index) => bytes[at + index] === byte);
}

function text(bytes: Uint8Array, at: number, word: string): boolean {
  return starts(
    bytes,
    at,
    Array.from(word, (char) => char.charCodeAt(0)),
  );
}

/** One stored file: its bytes and what its sniff gave. */
interface StoredBlob {
  bytes: Uint8Array<ArrayBuffer>;
  media: string;
  class: string;
  inline: boolean;
  meta: ReferenceMeta;
}

/** One project's references, by ID number, and the files they may name. */
export interface MockReferences {
  rows: Reference[];
  blobs: Map<string, StoredBlob>;
  /** Each file's address for an `<img>`, an `<audio>`, a `<video>` or a download, made once. */
  urls: Map<string, string>;
  /** The entries the documents' `references:` keys keep, in each document's written order. */
  promoted: InSpec[];
}

/** The most `in_spec` rows a list carries (`IN_SPEC_MAX`). */
export const IN_SPEC_MAX = 64;

/** The mock's `[[references.homes]]`: a kind's directory under its document's. */
const HOMES: Readonly<Record<string, string>> = { mechanic: "media" };

/** The homes its `.gitattributes` gives to LFS: every `media` directory under `docs/spec`, `filter=lfs`. */
const LFS_HOMES: ReadonlySet<string> = new Set(["media"]);

/** The extensions a promoted file's name may end in, by media type, in the daemon's order. */
const EXTENSIONS: Readonly<Record<string, readonly string[]>> = {
  "image/png": [".png"],
  "image/jpeg": [".jpg", ".jpeg"],
  "image/gif": [".gif"],
  "image/webp": [".webp"],
  "audio/wav": [".wav"],
  "audio/flac": [".flac"],
  "audio/ogg": [".ogg", ".oga", ".opus"],
  "audio/mpeg": [".mp3"],
  "video/mp4": [".mp4", ".m4v"],
  "video/webm": [".webm"],
  "model/gltf-binary": [".glb"],
  "application/x-blender": [".blend"],
  "application/pdf": [".pdf"],
};

/** The most characters an entry's text holds. */
const ENTRY_TEXT_MAX = 512;

function numberOf(id: string): number {
  return Number(id.replace(/^\D+-/, ""));
}

function bytesOf(base64: string): Uint8Array<ArrayBuffer> {
  return Uint8Array.from(atob(base64), (char) => char.charCodeAt(0));
}

/** The facts the mock reads: a PNG's width and height, nothing else. */
function factsOf(bytes: Uint8Array, media: string): ReferenceMeta {
  const meta: ReferenceMeta = { width: null, height: null, duration_ms: null, triangles: null };
  if (media === "image/png" && bytes.length >= 24) {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    meta.width = view.getUint32(16);
    meta.height = view.getUint32(20);
  }
  return meta;
}

/** A file's made-up 64-hex name: equal bytes, one name. */
function nameOf(bytes: Uint8Array): string {
  let sample = String(bytes.length);
  for (let at = 0; at < bytes.length; at += Math.max(1, Math.floor(bytes.length / 4096))) {
    sample += `,${String(bytes[at] ?? 0)}`;
  }
  return fakeHex(sample, 64);
}

function agentReady(stored: Pick<StoredBlob, "class" | "meta">, size: number): boolean {
  const { width, height } = stored.meta;
  return stored.class === "image" && width !== null && height !== null && Math.max(width, height) <= AGENT_EDGE && size <= AGENT_BYTES;
}

/** harbor-sim's material: under C-0003 lev's link, ivo's harbour image, tomas's sound lost with its host and a deleted one. */
export function mockReferences(slug: string, now: number): MockReferences {
  const store: MockReferences = { rows: [], blobs: new Map(), urls: new Map(), promoted: [] };
  if (slug !== "harbor-sim") {
    return store;
  }
  const at = (minutesAgo: number) => stamp(now, minutesAgo);
  const card = { kind: "card", id: "C-0003" };
  const person = (login: string) => ({ type: "human", role: null, model: null, run: null, login });
  const png = bytesOf(HARBOUR_PNG);
  const b3 = nameOf(png);
  const meta = factsOf(png, "image/png");
  store.blobs.set(b3, { bytes: png, media: "image/png", class: "image", inline: true, meta });
  const base = { on: card, on_rev: null, matters: null, url: null, name: null, b3: null, agent_b3: null, size: null, media: null, class: null, inline: null, meta: null, file: null, warning: null, deleted: null };
  store.rows.push(
    { ...base, id: "RF-0001", actor: "lev", author: person("lev"), kind: "link", title: "Fairway charts", url: "https://example.org/harbour/fairway", matters: "The night lights on the north side.", created_at: at(200) },
    { ...base, id: "RF-0002", actor: "ivo", author: person("ivo"), kind: "file", title: "Harbour at dusk", matters: "The light on the water, not the boats.", name: "harbour.png", b3, size: png.length, media: "image/png", class: "image", inline: true, meta, file: "stored", created_at: at(160) },
    {
      ...base,
      id: "RF-0003",
      actor: "tomas",
      author: person("tomas"),
      kind: "file",
      title: "Foghorn",
      // "Foghorn" in Russian, escaped: shown as written, any script (ADR-0072).
      matters: "\u0422\u0443\u043c\u0430\u043d\u043d\u044b\u0439 \u0433\u043e\u0440\u043d: two long blasts, then silence.",
      name: "foghorn.wav",
      b3: fakeHex("foghorn", 64),
      size: 1_572_864,
      media: "audio/wav",
      class: "audio",
      inline: true,
      meta: { width: null, height: null, duration_ms: 9000, triangles: null },
      file: "lost",
      created_at: at(120),
    },
    { ...base, id: "RF-0004", actor: "mira", author: person("mira"), kind: "file", title: null, size: 40_960, media: "application/pdf", class: "pdf", inline: false, created_at: at(90), deleted: { by: "mira", at: at(80) } },
  );
  // The night passage's key, written by an earlier promotion and by hand: C-0003's target inherits it.
  store.promoted.push(
    { node: "MEC-NIGHT-PASSAGE", document: "docs/spec/fairway/night.md", path: "docs/spec/fairway/media/buoys-at-night.png", matters: "The order of the lit buoys, not their colours." },
    // "Signal lights" in Russian, escaped: an entry's text is shown as written, any script (ADR-0072).
    { node: "MEC-NIGHT-PASSAGE", document: "docs/spec/fairway/night.md", path: "docs/spec/fairway/media/signal-lights.wav", matters: "\u0421\u0438\u0433\u043d\u0430\u043b\u044c\u043d\u044b\u0435 \u043e\u0433\u043d\u0438: two short, one long." },
    { node: "MEC-NIGHT-PASSAGE", document: "docs/spec/fairway/night.md", path: "docs/spec/fairway/media/chart-2019.pdf", matters: null },
  );
  return store;
}

/**
 * A target's page: the newest `limit` older than `before`, oldest first; on every page the entries
 * `documents` keep (their paths, each once, in order: a card's targets' documents, a node's own), at
 * most IN_SPEC_MAX, a note saying how many more.
 */
export function referencesIn(store: MockReferences, options: ReferencesOptions, kind: string, documents: readonly string[] = []): ReferenceList {
  const all = store.rows.filter((reference) => reference.on.id === options.on).sort((a, b) => numberOf(a.id) - numberOf(b.id));
  const older = options.before === undefined ? all : all.filter((reference) => numberOf(reference.id) < numberOf(options.before ?? ""));
  const page = older.slice(-(options.limit ?? PAGE));
  const first = page[0];
  const kept = [...new Set(documents)].flatMap((document) => store.promoted.filter((entry) => entry.document === document));
  const more = kept.length - IN_SPEC_MAX;
  return {
    on: { kind, id: options.on },
    total: all.length,
    references: structuredClone(page),
    next_before: first !== undefined && older.length > page.length ? first.id : null,
    notes: more > 0 ? [`${String(more)} more in the spec not shown: their documents list them`] : [],
    in_spec: structuredClone(kept.slice(0, IN_SPEC_MAX)),
  };
}

/** The document a REF lands in, as the mock's corpus resolves it, a section to its document; null when it names none. */
export function documentOf(corpus: MockCorpus, ref: string): { file: MockDocument; section: string | null } | null {
  const resolved = resolveRef(corpus, ref);
  if ("reason" in resolved) {
    return null;
  }
  const holder = resolved.holders[0];
  const file = holder === undefined ? undefined : corpus.documents.find((document) => document.path === holder.path);
  return file === undefined || holder === undefined ? null : { file, section: holder.isDocument ? null : holder.id };
}

/** A path's directory: all before its last `/`, `""` at the root. */
function directoryOf(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash < 0 ? "" : path.slice(0, slash);
}

/** Paths joined by `/`, empty parts left out. */
function joined(...parts: string[]): string {
  return parts.filter((part) => part !== "").join("/");
}

/** A refusal (exit 1): the route's 409 `{id, reason}`, the client's message its reason. */
function refused(reason: string): ClientError {
  return new ClientError({ status: 409, message: reason });
}

/** Cannot run (exit 2): the route's 503, the CLI's line. */
function cannot(line: string): ClientError {
  return new ClientError({ status: 503, message: `spec: ${line}` });
}

/** A size as the daemon's lines print it: one decimal past bytes, in binary units. */
function sizeLine(bytes: number): string {
  if (bytes < 1024) {
    return `${String(bytes)} B`;
  }
  return bytes < 1_048_576 ? `${(bytes / 1024).toFixed(1)} KiB` : `${(bytes / 1_048_576).toFixed(1)} MiB`;
}

/** An entry as the key writes it, quoted, `\` and `"` escaped. */
function quoted(entry: string): string {
  return `"${entry.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;
}

/** The unified hunk adding `entry` before the front-matter's closing fence, three lines of context; null when the file has no front-matter. */
function entryDiff(file: MockDocument, entry: string): string | null {
  const close = file.lines.indexOf("---", 1);
  if (file.lines[0] !== "---" || close < 0) {
    return null;
  }
  const from = Math.max(0, close - 3);
  const to = Math.min(file.lines.length, close + 4);
  const before = file.lines.slice(from, close).map((line) => ` ${line}`);
  const after = file.lines.slice(close, to).map((line) => ` ${line}`);
  const head = `@@ -${String(from + 1)},${String(to - from)} +${String(from + 1)},${String(to - from + 2)} @@`;
  return [`--- a/${file.path}`, `+++ b/${file.path}`, head, ...before, "+references:", `+  - ${quoted(entry)}`, ...after, ""].join("\n");
}

/**
 * `spec propose reference RF --to REF`, as the route answers it: the reference live, a file and
 * stored; the document's kind given a home; the name keeping its type's extension; the entry's text
 * within 512 characters; no live twin; then the `reference` row stored in `queue` and its review
 * document answered. Each refusal in the CLI's words: 404 an unknown reference or target, 409 a rule,
 * 503 what cannot run. Nothing else changes: the file and the key wait for a person's confirm.
 */
export function promoteIn(
  store: MockReferences,
  queue: MockProposal[],
  corpus: MockCorpus,
  project: string,
  id: string,
  body: PromoteBody,
  author: Author,
  at: string,
): StoredReview {
  const reference = stored(store, id);
  if (reference.deleted !== null) {
    throw refused(`${id} is deleted: nothing to promote`);
  }
  if (reference.kind === "link") {
    throw refused(`${id} is a link: only a file is promoted into git`);
  }
  const blob = reference.b3 === null ? undefined : store.blobs.get(reference.b3);
  if (reference.file !== "stored" || blob === undefined) {
    throw refused(`${id}'s file is not on this host: nothing to promote`);
  }
  if (body.rationale.trim() === "") {
    throw new ClientError({ status: 400, message: "spec: rationale: blank: give a value" });
  }
  const found = documentOf(corpus, body.to);
  if (found === null) {
    throw new ClientError({ status: 404, message: `\`${body.to}\` resolves to no ID and no alias` });
  }
  const { file, section } = found;
  const document = file.node.id ?? file.path;
  const kind = file.node.kind;
  if (kind === null) {
    throw cannot(`\`${file.path}\` has no kind: a [references] home is chosen by kind`);
  }
  const home = HOMES[kind];
  if (home === undefined) {
    throw cannot(`no [references] home takes kind \`${kind}\` (\`${document}\`): add one to [[references.homes]] in specengine.toml; nothing stored`);
  }
  const name = body.name ?? reference.name ?? "";
  const extensions = EXTENSIONS[blob.media] ?? [];
  if (name.includes("|") || name.startsWith(".") || name.endsWith(".") || name.endsWith(" ")) {
    throw refused(`name \`${name}\` holds a \`|\`, starts with \`.\` or ends in a space or \`.\`: give another with --name`);
  }
  if (!extensions.some((extension) => name.toLowerCase().endsWith(extension))) {
    throw refused(`name \`${name}\` does not end in an extension of ${blob.media} (${extensions.join(", ")}): give one with --name`);
  }
  const text = (body.matters ?? reference.matters ?? "").replace(/[\r\n]+/g, " ").trim();
  const characters = Array.from(text).length;
  if (characters > ENTRY_TEXT_MAX) {
    throw refused(`${id}'s what matters is ${String(characters)} characters: an entry holds at most 512; give one with --matters`);
  }
  const twin = queue.find(({ review }) => review.ref_id === id && review.target_id === document && (review.status === "open" || review.status === "changes_requested" || review.status === "approved"));
  if (twin !== undefined) {
    throw refused(`${id} is already proposed for \`${document}\` by ${twin.review.id} (${twin.review.status})`);
  }
  const relative = joined(home, name);
  const path = joined(directoryOf(file.path), relative);
  if (store.promoted.some((entry) => entry.document === file.path && entry.path === path)) {
    throw refused(`\`${relative}\` is already listed in \`${document}\`'s references`);
  }
  const entry = text === "" ? relative : `${relative} | ${text}`;
  const highest = queue.reduce((most, { review }) => Math.max(most, numberOf(review.id)), 0);
  const made = proposal({
    id: `PR-${String(highest + 1).padStart(4, "0")}`,
    project,
    kind: "reference",
    created_at: at,
    target_id: document,
    target_path: file.path,
    base_hash: `b3:${fakeHex(`${file.path}\n${file.lines.join("\n")}`, 64)}`,
    new_text: entry,
    patch_hash: `b3:${fakeHex(`${document}\n${entry}`, 64)}`,
    rationale: body.rationale,
    author,
    diff: entryDiff(file, entry),
    preview: "applies",
    revisions: [],
    ref_id: id,
    notes: [
      ...(section === null ? [] : [`\`${section}\` is a section of \`${document}\`: the file goes beside the document`]),
      `adds ${path}: ${id}, ${blob.media} ${sizeLine(blob.bytes.length)}, ${LFS_HOMES.has(home) ? "LFS" : "plain"}`,
    ],
  });
  queue.push(made);
  return made.review;
}

/** A hub's confirm of a `reference` proposal: its entry joins its document's key, the path root-relative. */
export function promotedIn(store: MockReferences, review: Pick<StoredReview, "target_id" | "target_path" | "new_text">): void {
  if (review.target_path === null || review.new_text === null) {
    return;
  }
  const split = review.new_text.indexOf(" | ");
  const relative = split < 0 ? review.new_text : review.new_text.slice(0, split);
  store.promoted.push({ node: review.target_id ?? review.target_path, document: review.target_path, path: joined(directoryOf(review.target_path), relative), matters: split < 0 ? null : review.new_text.slice(split + 3) });
}

function cancelled(): DOMException {
  return new DOMException("The upload was cancelled", "AbortError");
}

/** An upload: its bytes read, sniffed and kept, within the mock's 8 MiB. */
export async function putIn(store: MockReferences, blob: Blob, onProgress: UploadProgress, signal: AbortSignal): Promise<BlobStored> {
  if (signal.aborted) {
    throw cancelled();
  }
  onProgress(0, blob.size);
  // Judged on the size before a byte is read, as the daemon judges `Content-Length`.
  const kept = [...store.blobs.values()].reduce((sum, stored) => sum + stored.bytes.length, 0);
  if (kept + blob.size > MOCK_BYTES) {
    throw new ClientError({ status: 413, message: "refused: the mock keeps at most 8 MiB of uploads; reload the page to empty it" });
  }
  const bytes = new Uint8Array(await blob.arrayBuffer());
  // Cancel while the bytes were read.
  signal.throwIfAborted();
  const type = TYPES.find(([, , , test]) => test(bytes.subarray(0, 4096)));
  if (type === undefined) {
    throw new ClientError({ status: 415, message: "refused: the upload: not a type stored here (images PNG, JPEG, GIF, WebP; audio WAV, FLAC, Ogg, MP3; video MP4, WebM; models GLB, .blend; PDF)" });
  }
  const [media, cls, inline] = type;
  const b3 = nameOf(bytes);
  const existing = store.blobs.has(b3);
  const stored: StoredBlob = { bytes, media, class: cls, inline, meta: factsOf(bytes, media) };
  store.blobs.set(b3, stored);
  onProgress(blob.size, blob.size);
  return {
    b3,
    size: bytes.length,
    media,
    class: cls,
    inline,
    meta: stored.meta,
    agent_ready: agentReady(stored, bytes.length),
    needs_matters: MATTERS.has(cls),
    warning: media === "application/x-blender" ? BLEND_WARNING : null,
    stored: existing ? "existing" : "new",
  };
}

function stored(store: MockReferences, id: string): Reference {
  const found = store.rows.find((reference) => reference.id === id);
  if (found === undefined) {
    throw new ClientError({ status: 404, message: `no reference ${id}` });
  }
  return found;
}

/** A reference attached under `on`, by `actor`; `kind` what `on` names. */
export function attachIn(store: MockReferences, body: ReferenceAddBody, kind: string, actor: string, login: string | null, at: string): Reference {
  const highest = store.rows.reduce((most, reference) => Math.max(most, numberOf(reference.id)), 0);
  const base = {
    id: `RF-${String(highest + 1).padStart(4, "0")}`,
    on: { kind, id: body.on },
    on_rev: null,
    actor,
    author: login === null ? { type: "human", role: null, model: null, run: null } : { type: "human", role: null, model: null, run: null, login },
    title: body.title,
    matters: body.matters,
    created_at: at,
    deleted: null,
  };
  let made: Reference;
  if ("url" in body) {
    if (!/^https?:\/\/[^\s/]+/.test(body.url)) {
      throw new ClientError({ status: 400, message: `spec: url: ${body.url} is not an http:// or https:// address with a host` });
    }
    made = { ...base, kind: "link", url: body.url, name: null, b3: null, agent_b3: null, size: null, media: null, class: null, inline: null, meta: null, file: null, warning: null };
  } else {
    const file = store.blobs.get(body.b3);
    if (file === undefined) {
      throw new ClientError({ status: 404, message: `blob ${body.b3} is not stored here: upload it again` });
    }
    if (MATTERS.has(file.class) && (body.matters === null || body.matters.trim() === "")) {
      throw new ClientError({ status: 400, message: "spec: matters: required for audio, video, models and PDF: say what matters here" });
    }
    if (body.agent_b3 !== null) {
      if (agentReady(file, file.bytes.length)) {
        throw new ClientError({ status: 400, message: "spec: agent_b3: the image reaches agents as it is; it takes no copy" });
      }
      if (!store.blobs.has(body.agent_b3)) {
        throw new ClientError({ status: 404, message: `blob ${body.agent_b3} is not stored here: upload it again` });
      }
    }
    made = {
      ...base,
      kind: "file",
      url: null,
      name: body.name,
      b3: body.b3,
      agent_b3: body.agent_b3,
      size: file.bytes.length,
      media: file.media,
      class: file.class,
      inline: file.inline,
      meta: file.meta,
      file: "stored",
      warning: file.media === "application/x-blender" ? BLEND_WARNING : null,
    };
  }
  store.rows.push(made);
  return structuredClone(made);
}

/** A delete: the author's, or anyone's where `allowed`; the texts and hashes erased, a file no live reference names collected. */
export function deleteRefIn(store: MockReferences, id: string, actor: string, allowed: boolean, at: string): Reference {
  const reference = stored(store, id);
  if (reference.deleted !== null) {
    throw new ClientError({ status: 409, message: `${id} is deleted: nothing changed` });
  }
  if (!allowed && reference.actor !== actor) {
    throw new ClientError({ status: 403, message: `refused: ${id} is ${reference.actor}'s: only its author or an \`owner\` deletes it` });
  }
  const named = [reference.b3, reference.agent_b3];
  Object.assign(reference, { title: null, matters: null, url: null, name: null, b3: null, agent_b3: null, meta: null, file: null, deleted: { by: actor, at } });
  for (const b3 of named) {
    if (b3 !== null && !store.rows.some((row) => row.deleted === null && (row.b3 === b3 || row.agent_b3 === b3))) {
      store.blobs.delete(b3);
      const url = store.urls.get(b3);
      if (url !== undefined && url.startsWith("blob:")) {
        URL.revokeObjectURL(url);
      }
      store.urls.delete(b3);
    }
  }
  return structuredClone(reference);
}

/** A file's address: its bytes as an object URL (a data URL where the page has none); a file the mock does not hold, the daemon's path, which answers nothing here. */
export function urlIn(store: MockReferences, project: string, b3: string): string {
  const known = store.urls.get(b3);
  if (known !== undefined) {
    return known;
  }
  const file = store.blobs.get(b3);
  if (file === undefined) {
    return `/api/projects/${encodeURIComponent(project)}/blobs/${encodeURIComponent(b3)}`;
  }
  const url =
    typeof URL.createObjectURL === "function"
      ? URL.createObjectURL(new Blob([file.bytes], { type: file.media }))
      : `data:${file.media};base64,${btoa(Array.from(file.bytes, (byte) => String.fromCharCode(byte)).join(""))}`;
  store.urls.set(b3, url);
  return url;
}
