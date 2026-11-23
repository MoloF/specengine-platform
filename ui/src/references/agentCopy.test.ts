import { afterEach, describe, expect, it, vi } from "vitest";
import { fileOf, jpegBytes, pngBytes } from "../test/images";
import { AGENT_BYTES, AGENT_EDGE, agentCopyOf, aspectKept, copySize, COPY_QUALITIES, needsCopy, PIXEL_BUDGET } from "./agentCopy";
import { headerOf } from "./imageHeader";

// docs/features/work-references.md "The agent copy" and RF21, with the review of iteration 1: a picked
// image's size is read from its first bytes (the core's `blob::facts` rules), never by decoding it;
// within 1 568 px and 1 MiB nothing is decoded; past 64 megapixels nothing is decoded and no copy is
// made; else the browser decodes it straight to the copy's size (`resizeWidth`, `resizeHeight`),
// upright for an EXIF-turned JPEG, and encodes WebP at 0.85, 0.7, 0.55, the first within 1 MiB; JPEG
// where WebP is not written; none fits, none. The copy's size is one the core's own check accepts:
// a long strip's long edge is stepped down to a size that keeps its aspect. jsdom draws nothing:
// `createImageBitmap`, the 2D context and `toBlob` are stubbed.

const MIB = 1_048_576;

/**
 * The core's check, as `crates/specengine-core/src/blob.rs` `agent_copy_problem` writes it now, in
 * u128 (BigInt): within 1 %, or `off ≤ max(width, height)`, as stored or transposed. The test's own oracle.
 */
function coreAccepts(width: number, height: number, copyWidth: number, copyHeight: number): boolean {
  const within = (across: number, down: number) => {
    const signed = BigInt(copyWidth) * BigInt(down) - BigInt(copyHeight) * BigInt(across);
    const off = signed < 0n ? -signed : signed;
    return off * 100n <= BigInt(across) * BigInt(copyHeight) || off <= BigInt(Math.max(across, down));
  };
  return within(width, height) || within(height, width);
}

/** The 1 % clause alone, the core's rule before it took a pixel's rounding. */
function onePercent(width: number, height: number, copyWidth: number, copyHeight: number): boolean {
  const within = (across: number, down: number) => Math.abs(copyWidth * down - copyHeight * across) * 100 <= across * copyHeight;
  return within(width, height) || within(height, width);
}

/** No Cancel. */
const going = () => new AbortController().signal;

/** A decoded image as `createImageBitmap` gives it; `closed` counts its release. */
function bitmap(width: number, height: number) {
  const made = {
    width,
    height,
    closed: 0,
    close(): void {
      made.closed += 1;
    },
  };
  return made;
}

/** Stubs the browser's drawing: the decode (answering the size asked), the context's draws, and `toBlob` by `encode(type, quality)`. */
function stubDrawing(encode: (type: string, quality: number) => { type: string; size: number } | null) {
  const decoded: ReturnType<typeof bitmap>[] = [];
  const decode = vi.fn((_source: ImageBitmapSource, options?: ImageBitmapOptions) => {
    const made = bitmap(options?.resizeWidth ?? 0, options?.resizeHeight ?? 0);
    decoded.push(made);
    return Promise.resolve(made);
  });
  vi.stubGlobal("createImageBitmap", decode);
  const draws: unknown[][] = [];
  const sizes: [number, number][] = [];
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(function (this: HTMLCanvasElement) {
    sizes.push([this.width, this.height]);
    return {
      drawImage: (...args: unknown[]) => {
        draws.push(args.slice(1));
      },
    } as unknown as CanvasRenderingContext2D;
  });
  const asked: [string, number][] = [];
  vi.spyOn(HTMLCanvasElement.prototype, "toBlob").mockImplementation((callback, type, quality) => {
    asked.push([type ?? "", quality ?? 0]);
    const made = encode(type ?? "", quality ?? 0);
    callback(made === null ? null : new Blob([new Uint8Array(made.size)], { type: made.type }));
  });
  return { decode, decoded, draws, sizes, asked };
}

/** What `createImageBitmap` was asked to decode to, per call; each asked upright, by its EXIF orientation. */
function resized(decode: ReturnType<typeof stubDrawing>["decode"]): [number | undefined, number | undefined][] {
  for (const [, options] of decode.mock.calls) {
    expect(options?.imageOrientation).toBe("from-image");
  }
  return decode.mock.calls.map(([, options]) => [options?.resizeWidth, options?.resizeHeight]);
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("which image needs a copy, and its size", () => {
  it("past 1 568 px on the long edge or past 1 MiB, either way round", () => {
    expect([AGENT_EDGE, AGENT_BYTES, PIXEL_BUDGET]).toEqual([1568, MIB, 64_000_000]);
    expect([needsCopy(1568, 1000, MIB), needsCopy(1569, 10, 10), needsCopy(10, 1569, 10), needsCopy(1200, 800, MIB + 1)]).toEqual([false, true, true, true]);
  });

  it("keeps a photo's aspect at a long edge of 1 568, or the image's own when smaller", () => {
    expect([copySize(4032, 3024), copySize(3024, 4032), copySize(4000, 3000), copySize(1000, 800)]).toEqual([
      { width: 1568, height: 1176 },
      { width: 1176, height: 1568 },
      { width: 1568, height: 1176 },
      { width: 1000, height: 800 },
    ]);
  });

  it.each([
    [4096, 64, { width: 1568, height: 25 }],
    [2048, 32, { width: 1568, height: 25 }],
    [8192, 128, { width: 1568, height: 25 }],
    [16384, 256, { width: 1568, height: 25 }],
    [1600, 20, { width: 1568, height: 20 }],
    [64, 4096, { width: 25, height: 1568 }],
    [20, 1600, { width: 20, height: 1568 }],
  ])("gives a %ix%i strip its rounded 1568 size, which the core's rule accepts by its pixel clause though 1 % alone would refuse it (M: the 1 % clause alone)", (width, height, size) => {
    expect(copySize(width, height)).toEqual(size);
    expect(coreAccepts(width, height, size.width, size.height)).toBe(true);
    expect(onePercent(width, height, size.width, size.height)).toBe(false);
  });

  it("sizes a turned image as drawn, upright, and checks exactly that against its stored size", () => {
    expect(copySize(4000, 3000, true)).toEqual({ width: 1176, height: 1568 });
    expect(copySize(4096, 64, true)).toEqual({ width: 25, height: 1568 });
    expect(coreAccepts(4096, 64, 25, 1568)).toBe(true);
  });

  it("accepts exactly what the core's rule accepts, the transpose included", () => {
    for (const [width, height, copyWidth, copyHeight] of [
      [4032, 3024, 1568, 1176],
      [4032, 3024, 1176, 1568],
      [4096, 64, 1568, 25],
      [4096, 64, 1568, 26],
      [1600, 20, 1568, 20],
      [1600, 20, 1568, 22],
      [1000, 1000, 1000, 990],
      [1000, 1000, 1000, 989],
      [60000, 1, 1568, 1],
    ] as const) {
      expect([width, height, copyWidth, copyHeight, aspectKept(width, height, copyWidth, copyHeight)]).toEqual([width, height, copyWidth, copyHeight, coreAccepts(width, height, copyWidth, copyHeight)]);
    }
    expect(copySize(60000, 1)).toEqual({ width: 1568, height: 1 });
  });
});

describe("the size read from the first bytes, the core's rules", () => {
  it("reads PNG, a JPEG's frame past an EXIF thumbnail, its quarter turn, GIF and WebP's three chunks; anything else none", () => {
    expect(headerOf(pngBytes(4032, 3024))).toEqual({ width: 4032, height: 3024, turned: false });
    // The thumbnail's SOF (160x120) inside APP1 comes first in the bytes; the walk skips it (M: a byte scan for FF C0).
    expect(headerOf(jpegBytes(1600, 1200))).toEqual({ width: 1600, height: 1200, turned: false });
    expect(headerOf(jpegBytes(4000, 3000, 6))).toEqual({ width: 4000, height: 3000, turned: true });
    expect(headerOf(jpegBytes(4000, 3000, 3))).toEqual({ width: 4000, height: 3000, turned: false });
    const gif = new Uint8Array([...Array.from("GIF89a", (char) => char.charCodeAt(0)), 0x40, 0x01, 0xf0, 0x00]);
    expect(headerOf(gif)).toEqual({ width: 320, height: 240, turned: false });
    const riff = (chunk: string, body: number[]) => new Uint8Array([...Array.from(`RIFF\0\0\0\0WEBP${chunk}`, (char) => char.charCodeAt(0)), ...body]);
    // VP8 : the frame size, 14 bits each at 26 and 28.
    expect(headerOf(riff("VP8 ", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x20, 0x03, 0x58, 0x02]))).toEqual({ width: 800, height: 600, turned: false });
    // VP8L: 14 bits + 1 each from 21.
    const bits = 799 | (599 << 14);
    expect(headerOf(riff("VP8L", [0, 0, 0, 0, 0, bits & 0xff, (bits >> 8) & 0xff, (bits >> 16) & 0xff, (bits >>> 24) & 0xff]))).toEqual({ width: 800, height: 600, turned: false });
    // VP8X: 24 bits + 1 each at 24 and 27.
    expect(headerOf(riff("VP8X", [0, 0, 0, 0, 0, 0, 0, 0, 0x1f, 0x03, 0, 0x57, 0x02, 0]))).toEqual({ width: 800, height: 600, turned: false });
    expect([headerOf(new TextEncoder().encode("<!doctype html>")), headerOf(pngBytes(0, 10)), headerOf(new Uint8Array([0xff, 0xd8, 0xff]))]).toEqual([null, null, null]);
  });
});

describe("the copy the browser makes", () => {
  it("decodes a 4000x3000 image straight to 1568x1176 and takes the first WebP within 1 MiB", async () => {
    const { decode, decoded, draws, sizes, asked } = stubDrawing((type) => ({ type, size: 400 * 1024 }));
    const outcome = await agentCopyOf(fileOf(pngBytes(4000, 3000), "harbour.png", "image/png"), going());
    expect("copy" in outcome ? [outcome.copy.blob.type, outcome.copy.blob.size, outcome.copy.width, outcome.copy.height] : outcome).toEqual(["image/webp", 400 * 1024, 1568, 1176]);
    expect(resized(decode)).toEqual([[1568, 1176]]);
    expect(sizes).toEqual([[1568, 1176]]);
    expect(draws).toEqual([[0, 0, 1568, 1176]]);
    expect(asked).toEqual([["image/webp", 0.85]]);
    expect(decoded.map((made) => made.closed)).toEqual([1]);
  });

  it("decodes a 60000x60000 PNG declared in its header never, and makes no copy: past 64 megapixels (M: decoded before its size is read)", async () => {
    const { decode } = stubDrawing((type) => ({ type, size: 10 }));
    const outcome = await agentCopyOf(fileOf(pngBytes(60000, 60000), "huge.png", "image/png"), going());
    expect(outcome).toEqual({ why: "budget", width: 60000, height: 60000 });
    expect(decode).not.toHaveBeenCalled();
  });

  it("decodes an image within both limits never: none needed (M: every image decoded)", async () => {
    const { decode } = stubDrawing((type) => ({ type, size: 10 }));
    expect(await agentCopyOf(fileOf(pngBytes(1200, 800, 300 * 1024), "small.png", "image/png"), going())).toEqual({ why: "ready" });
    expect(decode).not.toHaveBeenCalled();
  });

  it("decodes nothing whose size its first bytes do not give", async () => {
    const { decode } = stubDrawing((type) => ({ type, size: 10 }));
    expect(await agentCopyOf(new Blob([new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])], { type: "image/png" }), going())).toEqual({ why: "unknown" });
    expect(decode).not.toHaveBeenCalled();
  });

  it("draws a JPEG its EXIF turns a quarter upright, its copy's edges swapped, which the core takes as the transpose", async () => {
    const { decode } = stubDrawing((type) => ({ type, size: 300 * 1024 }));
    const outcome = await agentCopyOf(fileOf(jpegBytes(4000, 3000, 6), "phone.jpg", "image/jpeg"), going());
    expect(resized(decode)).toEqual([[1176, 1568]]);
    expect("copy" in outcome ? [outcome.copy.width, outcome.copy.height, coreAccepts(4000, 3000, outcome.copy.width, outcome.copy.height)] : outcome).toEqual([1176, 1568, true]);
  });

  it("draws a 4096x64 strip at 1568x25, a size the core accepts", async () => {
    const { decode } = stubDrawing((type) => ({ type, size: 2 * 1024 }));
    await agentCopyOf(fileOf(pngBytes(4096, 64), "strip.png", "image/png"), going());
    expect(resized(decode)).toEqual([[1568, 25]]);
  });

  it("leaves a file that is no image alone, whatever its type says: nothing decoded, nothing said", async () => {
    const { decode } = stubDrawing((type) => ({ type, size: 10 }));
    const told = vi.fn();
    expect(await agentCopyOf(new Blob(["%PDF-1.7\n"], { type: "image/png" }), going(), told)).toEqual({ why: "notImage" });
    expect([decode.mock.calls.length, told.mock.calls.length]).toEqual([0, 0]);
  });

  it("hears Cancel before the decode and between encodings, making no copy (M: Cancel heard only after the copy)", async () => {
    const { decode } = stubDrawing((type) => ({ type, size: 10 }));
    const before = new AbortController();
    before.abort();
    expect(await agentCopyOf(fileOf(pngBytes(4000, 3000), "harbour.png", "image/png"), before.signal)).toEqual({ why: "cancelled" });
    expect(decode).not.toHaveBeenCalled();
    const midway = new AbortController();
    const { asked } = stubDrawing((type) => {
      midway.abort();
      return { type, size: 2 * MIB };
    });
    const told = vi.fn();
    expect(await agentCopyOf(fileOf(pngBytes(4000, 3000), "harbour.png", "image/png"), midway.signal, told)).toEqual({ why: "cancelled" });
    expect([asked.length, told.mock.calls.length]).toEqual([1, 1]);
  });

  it("steps the quality down, 0.85, 0.7, 0.55, until a copy fits", async () => {
    expect(COPY_QUALITIES).toEqual([0.85, 0.7, 0.55]);
    const { asked } = stubDrawing((type, quality) => ({ type, size: quality > 0.6 ? 2 * MIB : MIB }));
    const outcome = await agentCopyOf(fileOf(pngBytes(6000, 4000), "big.png", "image/png"), going());
    expect("copy" in outcome ? outcome.copy.blob.size : outcome).toBe(MIB);
    expect(asked).toEqual([
      ["image/webp", 0.85],
      ["image/webp", 0.7],
      ["image/webp", 0.55],
    ]);
  });

  it("encodes JPEG at the same steps where the browser writes no WebP", async () => {
    // A browser without a WebP encoder answers PNG, the canvas's default.
    const { asked } = stubDrawing((type) => (type === "image/webp" ? { type: "image/png", size: 3 * MIB } : { type, size: 700 * 1024 }));
    const outcome = await agentCopyOf(fileOf(pngBytes(2000, 2000), "square.png", "image/png"), going());
    expect("copy" in outcome ? [outcome.copy.blob.type, outcome.copy.width, outcome.copy.height] : outcome).toEqual(["image/jpeg", 1568, 1568]);
    expect(asked).toEqual([
      ["image/webp", 0.85],
      ["image/jpeg", 0.85],
    ]);
  });

  it("makes no copy when none fits within 1 MiB, or the browser cannot decode or draw it", async () => {
    const { asked } = stubDrawing((type) => ({ type, size: 2 * MIB }));
    expect(await agentCopyOf(fileOf(pngBytes(8000, 6000), "big.png", "image/png"), going())).toEqual({ why: "failed" });
    // WebP encoded but too large at every step: no JPEG tried.
    expect(asked.map(([type]) => type)).toEqual(["image/webp", "image/webp", "image/webp"]);
    vi.stubGlobal("createImageBitmap", vi.fn(() => Promise.reject(new DOMException("The source image could not be decoded.", "InvalidStateError"))));
    expect(await agentCopyOf(fileOf(pngBytes(4000, 3000), "broken.png", "image/png"), going())).toEqual({ why: "failed" });
    vi.stubGlobal("createImageBitmap", vi.fn(() => Promise.resolve(bitmap(1568, 1176))));
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    expect(await agentCopyOf(fileOf(pngBytes(4000, 3000), "plain.png", "image/png"), going())).toEqual({ why: "failed" });
  });

  it("copies an image within 1 568 px but past 1 MiB at its own size", async () => {
    const { decode } = stubDrawing((type) => ({ type, size: 300 * 1024 }));
    const outcome = await agentCopyOf(fileOf(pngBytes(1000, 800, MIB), "heavy.png", "image/png"), going());
    expect("copy" in outcome ? [outcome.copy.width, outcome.copy.height] : outcome).toEqual([1000, 800]);
    expect(resized(decode)).toEqual([[1000, 800]]);
  });
});
