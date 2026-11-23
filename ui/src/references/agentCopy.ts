import { headerOf, readHead, startsAsImage, type ImageHeader } from "./imageHeader";
import { AGENT_BYTES, AGENT_EDGE, COPY_TYPES, PIXEL_BUDGET } from "./labels";

// The agent copy (docs/features/work-references.md "The agent copy"; ADR-0088): an image reaches agents
// as MCP image content up to 1 568 px on its long edge and 1 MiB. A picked file's first bytes are read
// (./imageHeader.ts) and say whether it is an image at all and its size; nothing is decoded to learn
// either. An image already within both limits is not decoded, and one past 64 megapixels gets no
// copy: that cap is what bounds the decode (ADR-0086: a tab decoding a huge image at full size is a
// crash), `resizeWidth` and `resizeHeight` only letting a browser decode smaller where it can; the
// file is uploaded either way. Else the browser decodes it upright (`imageOrientation: "from-image"`,
// asked for, so a browser that cannot gives no copy rather than a distorted one) to the copy's size,
// draws it on a canvas and encodes WebP at 0.85, then 0.7, then 0.55, the first at most 1 MiB; where
// the browser does not encode WebP (its `toBlob` answering another type), JPEG at the same steps. The
// copy's size, as drawn, is one the core accepts (`agent_copy_problem`: within 1 % of the image's
// aspect, or within a pixel of it on one edge, as stored or transposed): the long edge at 1 568, the
// short one rounded, else the long edge stepped down until the core's rule holds. None fits: no
// copy, and the reference stores without one. Cancel is heard before the decode and between
// encodings. No server decodes a pixel.

export { AGENT_BYTES, AGENT_EDGE, PIXEL_BUDGET } from "./labels";

/** The qualities tried, in order: the first copy within AGENT_BYTES is taken. */
export const COPY_QUALITIES: readonly number[] = [0.85, 0.7, 0.55];

/** The copy's types, in order: JPEG only where the browser does not encode WebP. */
const { webp: WEBP, jpeg: JPEG } = COPY_TYPES;

/** A type the browser does not encode: its `toBlob` gave another, or none. */
const NOT_ENCODED = Symbol("NOT_ENCODED");

/** Whether an image of these dimensions and bytes needs a copy: past AGENT_EDGE on its long edge or past AGENT_BYTES. */
export function needsCopy(width: number, height: number, bytes: number): boolean {
  return Math.max(width, height) > AGENT_EDGE || bytes > AGENT_BYTES;
}

/**
 * The core's check of a copy's aspect (`crates/specengine-core/src/blob.rs` `agent_copy_problem`,
 * `within`), its rule as landed: with `off = |copy width × height − copy height × width|`, either
 * `off × 100 ≤ width × copy height` (within 1 %) or `off ≤ max(width, height)` (within a pixel on
 * one edge), for the image as stored or transposed (an EXIF turn). The UI takes the whole rule, so a
 * strip's rounded short edge (4096x64 to 1568x25) is accepted as the core accepts it.
 */
export function aspectKept(width: number, height: number, copyWidth: number, copyHeight: number): boolean {
  const within = (across: number, down: number) => {
    const off = Math.abs(copyWidth * down - copyHeight * across);
    return off * 100 <= across * copyHeight || off <= Math.max(across, down);
  };
  return within(width, height) || within(height, width);
}

/**
 * The copy's dimensions as drawn: upright (an EXIF-turned image's edges swapped), its long edge at
 * AGENT_EDGE (or its own when smaller), the short edge rounded; checked with the core's rule against
 * the stored size, exactly as sent, and when refused the long edge stepped down until it holds. Null
 * when no size does.
 */
export function copySize(width: number, height: number, turned = false): { width: number; height: number } | null {
  const long = Math.max(width, height);
  const short = Math.min(width, height);
  const wide = turned ? height >= width : width >= height;
  for (let edge = Math.min(long, AGENT_EDGE); edge >= 1; edge -= 1) {
    const other = Math.max(1, Math.round((edge * short) / long));
    const size = wide ? { width: edge, height: other } : { width: other, height: edge };
    if (aspectKept(width, height, size.width, size.height)) {
      return size;
    }
  }
  return null;
}

function encoded(canvas: HTMLCanvasElement, type: string, quality: number): Promise<Blob | null> {
  return new Promise((resolve) => {
    canvas.toBlob(resolve, type, quality);
  });
}

/** Cancel heard between two encodings. */
const CANCELLED = Symbol("CANCELLED");

/** The first encoding of `type` within AGENT_BYTES, null when none fits, NOT_ENCODED when the browser does not write `type`, CANCELLED when Cancel is heard first. */
async function fitting(canvas: HTMLCanvasElement, type: string, signal: AbortSignal): Promise<Blob | null | typeof NOT_ENCODED | typeof CANCELLED> {
  for (const quality of COPY_QUALITIES) {
    if (signal.aborted) {
      return CANCELLED;
    }
    const blob = await encoded(canvas, type, quality);
    if (blob?.type !== type) {
      return NOT_ENCODED;
    }
    if (blob.size <= AGENT_BYTES) {
      return blob;
    }
  }
  return null;
}

/** A copy made, with its dimensions as drawn (upright) to say. */
export interface AgentCopy {
  blob: Blob;
  width: number;
  height: number;
}

/**
 * Why no copy was made: `notImage`, the first bytes are no PNG, JPEG, GIF or WebP; `ready`, none
 * needed (within both limits); `unknown`, an image whose size its first bytes do not give;
 * `budget`, past PIXEL_BUDGET, never decoded; `shape`, no size the core's rule accepts; `failed`, the
 * browser did not decode it upright, draw or encode it within AGENT_BYTES; `cancelled`, Cancel
 * heard on the way.
 */
export type NoCopy = { why: "notImage" | "ready" | "unknown" | "shape" | "failed" | "cancelled" } | { why: "budget"; width: number; height: number };

/** A copy, or why none. */
export type CopyOutcome = { copy: AgentCopy } | NoCopy;

/** Decodes the image upright to `width` × `height` and encodes it within AGENT_BYTES; null when that fails, `cancelled` when Cancel is heard. */
async function drawn(file: Blob, width: number, height: number, signal: AbortSignal): Promise<AgentCopy | null | "cancelled"> {
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(file, { imageOrientation: "from-image", resizeWidth: width, resizeHeight: height, resizeQuality: "high" });
  } catch {
    return null;
  }
  const canvas = document.createElement("canvas");
  try {
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (context === null) {
      return null;
    }
    context.drawImage(bitmap, 0, 0, width, height);
    const webp = await fitting(canvas, WEBP, signal);
    if (webp === CANCELLED) {
      return "cancelled";
    }
    const jpeg = webp === NOT_ENCODED ? await fitting(canvas, JPEG, signal) : webp;
    if (jpeg === CANCELLED) {
      return "cancelled";
    }
    const made = jpeg;
    return made === NOT_ENCODED || made === null ? null : { blob: made, width, height };
  } catch {
    return null;
  } finally {
    bitmap.close();
    // The canvas's pixels go now, not at the next collection.
    canvas.width = 0;
    canvas.height = 0;
  }
}

/**
 * The copy of an image whose stored size `header` gives, or why none; `onDecode` is told just before
 * a decode starts (the dialog says it is making a copy then, and only then).
 */
export async function copyFor(file: Blob, header: ImageHeader | null, signal: AbortSignal, onDecode: () => void = () => undefined): Promise<CopyOutcome> {
  if (header === null) {
    return { why: "unknown" };
  }
  const { width, height, turned } = header;
  if (!needsCopy(width, height, file.size)) {
    return { why: "ready" };
  }
  if (width * height > PIXEL_BUDGET) {
    return { why: "budget", width, height };
  }
  // As drawn: a turned image upright, its edges swapped, the core's rule checked on what is sent.
  const size = copySize(width, height, turned);
  if (size === null) {
    return { why: "shape" };
  }
  if (signal.aborted) {
    return { why: "cancelled" };
  }
  onDecode();
  const made = await drawn(file, size.width, size.height, signal);
  if (made === "cancelled") {
    return { why: "cancelled" };
  }
  return made === null ? { why: "failed" } : { copy: made };
}

/**
 * The copy of a picked file for agents, or why none: its first bytes read, a file that is no
 * PNG, JPEG, GIF or WebP left alone (`notImage`, said nowhere), an image's size from them, then
 * `copyFor`. Never throws.
 */
export async function agentCopyOf(file: Blob, signal: AbortSignal, onDecode?: () => void): Promise<CopyOutcome> {
  try {
    const head = await readHead(file);
    if (!startsAsImage(head)) {
      return { why: "notImage" };
    }
    return await copyFor(file, headerOf(head), signal, onDecode);
  } catch {
    return { why: "unknown" };
  }
}
