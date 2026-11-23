import { describe, expect, it } from "vitest";
import { jpegBytes, pngBytes } from "../test/images";
import { headerOf, HEADER_READ, startsAsImage } from "./imageHeader";

// The size read from a file's first bytes (docs/features/work-references.md "Facts"; the review of
// iteration 2): bad, cut and hostile headers each give null or the right size, never a throw and
// never a walk past the core's 1 MiB. The cases are the reviewer's probe.

const MIB = 1_048_576;

/** A JPEG whose `APP1` holds `tiff` as its EXIF, then a frame of 4000x3000. */
function withExif(tiff: number[]): Uint8Array {
  const app1 = [0x45, 0x78, 0x69, 0x66, 0, 0, ...tiff];
  const sof = [0xff, 0xc0, 0, 17, 8, 0x0b, 0xb8, 0x0f, 0xa0];
  return new Uint8Array([0xff, 0xd8, 0xff, 0xe1, (app1.length + 2) >> 8, (app1.length + 2) & 0xff, ...app1, ...sof]);
}

/** A TIFF header in one byte order: its IFD at `offset`, `count` entries given as `entries`. */
function tiff(order: "II" | "MM", offset: number, count: number, entries: number[][] = []): number[] {
  const little = order === "II";
  const u16 = (value: number) => (little ? [value & 0xff, value >> 8] : [value >> 8, value & 0xff]);
  const u32 = (value: number) => (little ? [value & 0xff, (value >> 8) & 0xff, (value >> 16) & 0xff, value >>> 24] : [value >>> 24, (value >> 16) & 0xff, (value >> 8) & 0xff, value & 0xff]);
  return [...Array.from(order, (char) => char.charCodeAt(0)), ...u16(42), ...u32(offset), ...u16(count), ...entries.flatMap((entry) => entry.flatMap((value, index) => (index === 2 ? u32(value) : u16(value))))];
}

/** One orientation entry: tag 0x0112, `type`, count 1, the value then two zero bytes. */
const orientation = (type: number, value: number) => [0x0112, type, 1, value, 0];

/** The size, or what was thrown: no case may throw. */
function read(bytes: Uint8Array): unknown {
  try {
    return headerOf(bytes);
  } catch (error) {
    return `threw ${String(error)}`;
  }
}

describe("hostile and cut headers (the reviewer's probe)", () => {
  it("reads at most the core's walk: HEADER_READ is 1 MiB and a frame header", () => {
    expect(HEADER_READ).toBe(MIB + 16);
  });

  it.each([
    ["0xFF fill to 1 MiB", (() => {
      const bytes = new Uint8Array(MIB + 16).fill(0xff);
      bytes[1] = 0xd8;
      return bytes;
    })()],
    ["restart markers to 1 MiB", (() => {
      const bytes = new Uint8Array(MIB + 16);
      bytes.set([0xff, 0xd8]);
      for (let at = 2; at + 1 < bytes.length; at += 2) {
        bytes[at] = 0xff;
        bytes[at + 1] = 0xd0;
      }
      return bytes;
    })()],
    ["a segment of length 0", new Uint8Array([0xff, 0xd8, 0xff, 0xe0, 0, 0, 0xff, 0xc0])],
    ["a segment of length 1", new Uint8Array([0xff, 0xd8, 0xff, 0xe0, 0, 1, 0xff, 0xc0])],
    ["a cut SOF", new Uint8Array([0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, 0x01])],
    ["an APP1 longer than the file", new Uint8Array([0xff, 0xd8, 0xff, 0xe1, 0xff, 0xff, 0x45, 0x78, 0x69, 0x66, 0, 0, 0x4d, 0x4d, 0, 42, 0, 0, 0, 8, 0, 1])],
    ["a cut PNG", new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13])],
    ["a cut WebP", new TextEncoder().encode("RIFF\0\0\0\0WEBPVP8X")],
    ["an empty file", new Uint8Array(0)],
    ["a JPEG's start alone", new Uint8Array([0xff, 0xd8, 0xff])],
  ])("gives none for %s, never a throw", (_name, bytes) => {
    expect(read(bytes)).toBeNull();
  });

  it.each(["II", "MM"] as const)("reads the frame past an EXIF whose IFD offset is 0xFFFFFFF0, or whose entry count is 65 535 (%s), unturned", (order) => {
    expect(read(withExif(tiff(order, 0xfffffff0, 0)))).toEqual({ width: 4000, height: 3000, turned: false });
    expect(read(withExif(tiff(order, 8, 65535)))).toEqual({ width: 4000, height: 3000, turned: false });
  });

  it.each(["II", "MM"] as const)("reads orientation 6 as a quarter turn in either byte order (%s); typed other than SHORT, not", (order) => {
    expect(read(withExif(tiff(order, 8, 1, [orientation(3, 6)])))).toEqual({ width: 4000, height: 3000, turned: true });
    expect(read(withExif(tiff(order, 8, 1, [orientation(3, 1)])))).toEqual({ width: 4000, height: 3000, turned: false });
    // A LONG-typed entry is no orientation as the TIFF writes it (M: the type unchecked).
    expect(read(withExif(tiff(order, 8, 1, [orientation(4, 6)])))).toEqual({ width: 4000, height: 3000, turned: false });
  });

  it("tells an image's first bytes from anything else's", () => {
    expect([startsAsImage(pngBytes(1, 1)), startsAsImage(jpegBytes(1, 1)), startsAsImage(new TextEncoder().encode("GIF89a")), startsAsImage(new TextEncoder().encode("RIFF\0\0\0\0WEBP"))]).toEqual([true, true, true, true]);
    expect([startsAsImage(new TextEncoder().encode("%PDF-1.7")), startsAsImage(new TextEncoder().encode("RIFF\0\0\0\0WAVE")), startsAsImage(new Uint8Array(0))]).toEqual([false, false, false]);
  });
});
