import type { ApiError, BlobStored, Reference, ReferenceMeta } from "../api/types";
import type { Translate } from "../i18n";

// The daemon's words a reference carries, told apart here alone (docs/features/work-references.md
// "Types", "Documents", "UI"; policy.test.ts): its kind, its class, whether its file is stored, which
// media types a tile previews, which classes need "what matters", and the sizes and facts as words.
// The daemon judges every upload and every write; nothing here judges a file, only how it is shown.

/** A reference's two kinds, the daemon's words, each marked whether it is a link: a URL, never fetched. */
const KINDS = { file: false, link: true } as const;

/** The types the browser's agent copy is encoded in, in order (`docs/features/work-references.md` "The agent copy"). */
export const COPY_TYPES = { webp: "image/webp", jpeg: "image/jpeg" } as const;

/** The long edge an image reaches agents within, the core's `AGENT_EDGE`. */
export const AGENT_EDGE = 1568;

/** The bytes an image reaches agents within, the core's `AGENT_BYTES`. */
export const AGENT_BYTES = 1_048_576;

/** The most pixels an image may have for this browser to decode it: into a copy, or into the viewer's `<img>`. */
export const PIXEL_BUDGET = 64_000_000;

/** The bytes past which an image of unknown size is not drawn in the viewer, only offered for download. */
const VIEW_BYTES_UNKNOWN = 32 * 1_048_576;

/** The stored classes, the core's table (`docs/features/work-references.md` "Types"). */
const CLASSES = { image: "image", audio: "audio", video: "video", model: "model", pdf: "pdf" } as const;

/** A class this build knows; another word is shown raw. */
type KnownClass = keyof typeof CLASSES;

/** A file the store holds now; `lost` and any other word draw no preview. */
const STORED = "stored";

/** The raster images the daemon serves inline, the only ones an `<img>` draws (`docs/features/work-references.md` "Types"). */
const RASTER: readonly string[] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/** The sounds the daemon serves inline, the only ones an `<audio>` plays. */
const SOUNDS: readonly string[] = ["audio/wav", "audio/flac", "audio/ogg", "audio/mpeg"];

/** The videos the daemon serves inline, the only ones a `<video>` plays. */
const VIDEOS: readonly string[] = ["video/mp4", "video/webm"];

/** The classes whose "what matters" is required (`docs/features/work-references.md` D10). */
const MATTERS: readonly string[] = [CLASSES.audio, CLASSES.video, CLASSES.model, CLASSES.pdf];

/** The most bytes a file has, the daemon's `BLOB_FILE_MAX`: a larger pick is refused before a byte is sent. */
export const FILE_MAX = 268_435_456;

/** What the picker offers first; any other file can still be picked, and the daemon judges it by its bytes. */
export const PICKER_ACCEPT = [...RASTER, ...SOUNDS, "audio/x-wav", ...VIDEOS, "model/gltf-binary", ".glb", ".blend", "application/pdf"].join(",");

/** How a tile shows a reference: a deleted one's tombstone, a link, a file lost with its host, a preview, or a download. */
export type Shown = "deleted" | "link" | "lost" | "image" | "audio" | "video" | "download";

/** Whether a tile previews its file (an image, a sound, a video), so a download stands beside the preview. */
export function previewed(shown: Shown): boolean {
  return shown === "image" || shown === "audio" || shown === "video";
}

/**
 * Delete on a live reference (`docs/features/work-references.md` "UI", Delete): on (`true`) anywhere
 * locally (`login` null: the dialog is the consent), on a hub where the login is its author's or the
 * hub's `allowed` lists `delete_reference` (`allows`, unknown reading on); else off, `ActOff` beside
 * it. The daemon judges the call either way.
 */
export function deleteStands(reference: Pick<Reference, "author">, login: string | null, allows: boolean): boolean {
  return login === null || allows || reference.author.login === login;
}

/** Whether a reference is a link: a URL, never fetched. */
export function isLink(reference: Pick<Reference, "kind">): boolean {
  return Object.hasOwn(KINDS, reference.kind) && KINDS[reference.kind as keyof typeof KINDS];
}

/**
 * How a tile shows a reference: an image only when it is one of the four raster types and served
 * inline, a sound or a video only when the daemon's table names it and serves it inline; a model, a
 * PDF, a `.blend` and anything else a download link; a file the store does not hold, lost.
 */
export function shownAs(reference: Reference): Shown {
  if (reference.deleted !== null) {
    return "deleted";
  }
  if (isLink(reference)) {
    return "link";
  }
  if (reference.file !== STORED || reference.b3 === null) {
    return "lost";
  }
  const media = reference.media ?? "";
  if (reference.inline === true && reference.class === CLASSES.image && RASTER.includes(media)) {
    return "image";
  }
  if (reference.inline === true && reference.class === CLASSES.audio && SOUNDS.includes(media)) {
    return "audio";
  }
  if (reference.inline === true && reference.class === CLASSES.video && VIDEOS.includes(media)) {
    return "video";
  }
  return "download";
}

/**
 * Whether a tile offers "Promote to canon…" (docs/canon/work-references-promotion-surfaces.md "UI"): a live file
 * the store holds now; never a link (D14), a deleted reference or a file lost with its host. The
 * daemon judges the promotion either way.
 */
export function promotable(reference: Reference): boolean {
  const shown = shownAs(reference);
  return shown !== "deleted" && shown !== "link" && shown !== "lost";
}

/** A class in the catalog's words; another word raw. */
export function classWord(value: string, t: Translate): string {
  return Object.hasOwn(CLASSES, value) ? t(`references.class.${value as KnownClass}`) : value;
}

/** Whether "what matters" is required of a class. */
export function needsMatters(value: string | null): boolean {
  return value !== null && MATTERS.includes(value);
}

/**
 * A picked file's class guessed from the browser's type, only to mark "what matters" required before
 * the upload answers (`docs/features/work-references.md` "UI"); the daemon's sniff decides. Null when
 * the browser names no type it knows.
 */
export function guessedClass(type: string): string | null {
  const major = type.split("/")[0] ?? "";
  if (major === CLASSES.image || major === CLASSES.audio || major === CLASSES.video) {
    return major;
  }
  if (type === "application/pdf") {
    return CLASSES.pdf;
  }
  return type === "model/gltf-binary" ? CLASSES.model : null;
}

/**
 * What a raster image's tile draws as its thumbnail: its agent copy; else the file itself when it
 * reaches agents as it is (its size known, within 1 568 px on the long edge and 1 MiB), so a tile
 * never pulls a large original; else null, the file loaded only when the person opens it.
 */
export function thumbnailOf(reference: Pick<Reference, "b3" | "agent_b3" | "meta" | "size">): string | null {
  if (reference.agent_b3 !== null) {
    return reference.agent_b3;
  }
  const width = reference.meta?.width ?? null;
  const height = reference.meta?.height ?? null;
  const ready = width !== null && height !== null && Math.max(width, height) <= AGENT_EDGE && reference.size !== null && reference.size <= AGENT_BYTES;
  return ready ? reference.b3 : null;
}

/**
 * Whether the viewer may draw the file itself: within PIXEL_BUDGET by its facts; of unknown size, at
 * most 32 MiB. Past either, the viewer shows its facts and Download alone, so opening it never
 * decodes hundreds of megabytes in the tab (ADR-0086).
 */
export function viewable(reference: Pick<Reference, "meta" | "size">): boolean {
  const width = reference.meta?.width ?? null;
  const height = reference.meta?.height ?? null;
  if (width !== null && height !== null) {
    return width * height <= PIXEL_BUDGET;
  }
  return reference.size !== null && reference.size <= VIEW_BYTES_UNKNOWN;
}

/**
 * The size a thumbnail's `<img>` reserves before it loads, or null: the facts give the stored size,
 * which an EXIF turn swaps on screen, so only a PNG's or a GIF's own file states it; a copy's size
 * and a JPEG's or a WebP's are left to the image.
 */
export function thumbnailSize(reference: Pick<Reference, "agent_b3" | "media" | "meta">): { width: number; height: number } | null {
  const width = reference.meta?.width ?? null;
  const height = reference.meta?.height ?? null;
  const upright = reference.media === "image/png" || reference.media === "image/gif";
  return reference.agent_b3 === null && upright && width !== null && height !== null ? { width, height } : null;
}

/**
 * Whether an attach's 400 refuses its agent copy, `agent_b3: …` (the CLI's line, `spec: ` before it
 * on the daemon): the attach is posted once more without the copy, so the file still lands.
 */
export function copyRefused({ status, message }: ApiError): boolean {
  return status === 400 && /^(spec: )?agent_b3: /.test(message);
}

/** Whether a stored blob takes an agent copy: an image the daemon does not find agent-ready as it is. */
export function takesCopy(stored: BlobStored): boolean {
  return stored.class === CLASSES.image && !stored.agent_ready;
}

/** Whether an attach's 404 says a named file is no longer stored (`blob <b3> is not stored here: upload it again`): it is uploaded once more. */
export function blobGone({ status, message }: ApiError, hashes: readonly string[]): boolean {
  return status === 404 && hashes.some((hash) => hash !== "" && message.includes(hash));
}

/** Whether a link's address is one a page may open: `http:` or `https:`; any other is shown as text alone. */
export function isWebAddress(url: string): boolean {
  try {
    const { protocol } = new URL(url);
    return protocol === "http:" || protocol === "https:";
  } catch {
    return false;
  }
}

const KIB = 1024;
const MIB = KIB * 1024;
const GIB = MIB * 1024;

/** A size in binary units, one decimal past bytes, as the daemon's lines print it. */
export function sizeWords(bytes: number, t: Translate): string {
  if (bytes < KIB) {
    return t("references.size.bytes", { size: String(bytes) });
  }
  if (bytes < MIB) {
    return t("references.size.kib", { size: (bytes / KIB).toFixed(1) });
  }
  if (bytes < GIB) {
    return t("references.size.mib", { size: (bytes / MIB).toFixed(1) });
  }
  return t("references.size.gib", { size: (bytes / GIB).toFixed(1) });
}

/** A file's facts as words, the known ones alone: its dimensions, its duration, its triangles. */
export function factWords(meta: ReferenceMeta | null, t: Translate): string[] {
  if (meta === null) {
    return [];
  }
  const words: string[] = [];
  if (meta.width !== null && meta.height !== null) {
    words.push(t("references.facts.dimensions", { width: String(meta.width), height: String(meta.height) }));
  }
  if (meta.duration_ms !== null) {
    words.push(t("references.facts.seconds", { seconds: (meta.duration_ms / 1000).toFixed(1) }));
  }
  if (meta.triangles !== null) {
    words.push(t("references.facts.triangles", { count: meta.triangles }));
  }
  return words;
}
