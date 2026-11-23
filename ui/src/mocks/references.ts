import { ClientError, type ReferencesOptions, type UploadProgress } from "../api/client";
import type { BlobStored, Reference, ReferenceAddBody, ReferenceList, ReferenceMeta } from "../api/types";
import { fakeHex, stamp } from "./build";

// The mock's references (docs/features/work-references.md "Data", "HTTP"; ADR-0088): harbor-sim's
// material under a task, held in memory until a reload, answered as `spec serve` would: a list oldest
// first in pages of 100 by ID, the newest page first; an upload's bytes sniffed by their first bytes
// (a refusal naming "the upload", as a PUT carries no name),
// kept in memory up to 8 MiB in all, named by a made-up 64-hex digest of their bytes (not BLAKE3, so
// equal bytes are one file); an attach and a delete refused in the CLI's words. A file a seed names
// with no bytes reads `lost`.

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
}

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
  const store: MockReferences = { rows: [], blobs: new Map(), urls: new Map() };
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
  return store;
}

/** A target's page: the newest `limit` older than `before`, oldest first. */
export function referencesIn(store: MockReferences, options: ReferencesOptions, kind: string): ReferenceList {
  const all = store.rows.filter((reference) => reference.on.id === options.on).sort((a, b) => numberOf(a.id) - numberOf(b.id));
  const older = options.before === undefined ? all : all.filter((reference) => numberOf(reference.id) < numberOf(options.before ?? ""));
  const page = older.slice(-(options.limit ?? PAGE));
  const first = page[0];
  return {
    on: { kind, id: options.on },
    total: all.length,
    references: structuredClone(page),
    next_before: first !== undefined && older.length > page.length ? first.id : null,
    notes: [],
  };
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
