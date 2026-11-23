// Image files as tests build them: only the first bytes a size is read from (docs/features/work-references.md
// "Facts"), then filler, so no test holds a real picture and nothing here is ever decoded.

function concat(parts: readonly (readonly number[] | Uint8Array)[]): Uint8Array<ArrayBuffer> {
  const length = parts.reduce((sum, part) => sum + part.length, 0);
  const out = new Uint8Array(length);
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
}

const be16 = (value: number) => [(value >> 8) & 0xff, value & 0xff];
const be32 = (value: number) => [(value >>> 24) & 0xff, (value >>> 16) & 0xff, (value >>> 8) & 0xff, value & 0xff];
const ascii = (word: string) => Array.from(word, (char) => char.charCodeAt(0));

/** A PNG's signature and `IHDR` of `width` × `height`, then `padding` zero bytes. */
export function pngBytes(width: number, height: number, padding = 0): Uint8Array<ArrayBuffer> {
  return concat([[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, ...ascii("IHDR"), ...be32(width), ...be32(height), 8, 2, 0, 0, 0], new Uint8Array(padding)]);
}

/**
 * A JPEG: SOI, an `APP1` holding EXIF whose first IFD names `orientation` (when given) and, inside
 * the segment, a thumbnail's own SOF0 of 160x120 the walk must skip, then the frame's SOF0 of
 * `width` × `height`, then SOS.
 */
export function jpegBytes(width: number, height: number, orientation?: number): Uint8Array<ArrayBuffer> {
  const entries = orientation === undefined ? [] : [[...be16(0x0112), ...be16(3), ...be32(1), ...be16(orientation), 0, 0]];
  const tiff = [...ascii("MM"), ...be16(42), ...be32(8), ...be16(entries.length), ...entries.flat(), ...be32(0)];
  const thumbnail = [0xff, 0xd8, 0xff, 0xc0, ...be16(17), 8, ...be16(120), ...be16(160), 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1];
  const app1 = [...ascii("Exif"), 0, 0, ...tiff, ...thumbnail];
  const sof = [0xff, 0xc0, ...be16(17), 8, ...be16(height), ...be16(width), 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1];
  return concat([[0xff, 0xd8, 0xff, 0xe1, ...be16(app1.length + 2), ...app1], sof, [0xff, 0xda, 0, 2]]);
}

/** A picked file of these bytes. */
export function fileOf(bytes: Uint8Array<ArrayBuffer>, name: string, type: string): File {
  return new File([bytes], name, { type });
}
